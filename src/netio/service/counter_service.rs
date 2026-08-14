use crate::db::counter_store::make_counter_key;
use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;
use crate::util::date;

/// Get real-time counter value for a single object + counter name.
pub fn counter_real_time(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };

    let obj_hash = match table.get("objHash") {
        Some(Value::Decimal(v)) => *v as i32,
        _ => 0,
    };
    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };

    if let Some(value) = ctx.cache.counter.get(obj_hash, &counter, 1) {
        let mut result = std::collections::HashMap::new();
        result.insert("objHash".into(), Value::Decimal(obj_hash as i64));
        result.insert("counter".into(), Value::Text(counter));
        result.insert("value".into(), value);

        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    }
    Ok(())
}

/// Get real-time counter values for ALL objects for a given counter name.
pub fn counter_real_time_all(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };

    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };

    let all = ctx.cache.counter.get_all_for_counter(&counter, 1);
    for (obj_hash, value) in all {
        let mut result = std::collections::HashMap::new();
        result.insert("objHash".into(), Value::Decimal(obj_hash as i64));
        result.insert("counter".into(), Value::Text(counter.clone()));
        result.insert("value".into(), value);

        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    }
    Ok(())
}

/// Get real-time values for multiple counters for a single object.
pub fn counter_real_time_multi(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };

    let obj_hash = match table.get("objHash") {
        Some(Value::Decimal(v)) => *v as i32,
        _ => 0,
    };

    let counters: Vec<String> = match table.get("counter") {
        Some(Value::List(list)) => {
            list.iter().filter_map(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            }).collect()
        }
        _ => Vec::new(),
    };

    let mut result = std::collections::HashMap::new();
    result.insert("objHash".into(), Value::Decimal(obj_hash as i64));

    for counter in &counters {
        if let Some(value) = ctx.cache.counter.get(obj_hash, counter, 1) {
            result.insert(counter.clone(), value);
        }
    }

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    Ok(())
}

// ─── Historical counter handlers ───

/// Helper: read a counter record from DB and build time[]/value[] MapPack.
fn build_daily_counter_response(
    ctx: &ServiceContext,
    date_str: &str,
    obj_hash: i32,
    counter: &str,
) -> Option<std::collections::HashMap<String, Value>> {
    let container = ctx.db_manager.get_or_create(date_str).ok()?;
    let key = make_counter_key(obj_hash, counter);
    let record = container.counter.read(&key).ok()??;

    let stime = date::date_millis(date_str);
    let bucket_count = record.values.len();

    // 5-minute interval in ms
    let interval: i64 = 300_000;

    let mut time_list = Vec::with_capacity(bucket_count);
    let mut value_list = Vec::with_capacity(bucket_count);
    for (i, &v) in record.values.iter().enumerate() {
        time_list.push(Value::Decimal(stime + interval * i as i64));
        value_list.push(Value::Double(v));
    }

    let mut table = std::collections::HashMap::new();
    table.insert("objHash".into(), Value::Decimal(obj_hash as i64));
    table.insert("time".into(), Value::List(time_list));
    table.insert("value".into(), Value::List(value_list));
    Some(table)
}

/// Helper: extract common params from MapPack table.
fn extract_map_params(table: &std::collections::HashMap<String, Value>) -> (String, i32, String) {
    let date_str = match table.get("date") {
        Some(Value::Text(s)) => s.clone(),
        _ => date::yyyymmdd_today(),
    };
    let obj_hash = match table.get("objHash") {
        Some(Value::Decimal(v)) => *v as i32,
        _ => 0,
    };
    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    (date_str, obj_hash, counter)
}

/// COUNTER_PAST_DATE: Get daily counter for a specific object + counter on a past date.
pub fn counter_past_date(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };
    let (date_str, obj_hash, counter) = extract_map_params(&table);

    if let Some(result) = build_daily_counter_response(ctx, &date_str, obj_hash, &counter) {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    }
    Ok(())
}

/// COUNTER_TODAY: Get today's counter for a specific object.
pub fn counter_today(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };
    let (_, obj_hash, counter) = extract_map_params(&table);
    let today = date::yyyymmdd_today();

    if let Some(result) = build_daily_counter_response(ctx, &today, obj_hash, &counter) {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    }
    Ok(())
}

/// COUNTER_PAST_DATE_ALL: Get daily counter for ALL objects of a type on a past date.
pub fn counter_past_date_all(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };
    let date_str = match table.get("date") {
        Some(Value::Text(s)) => s.clone(),
        _ => date::yyyymmdd_today(),
    };
    let obj_type = match table.get("objType") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };

    let objects = ctx.cache.object.get_live_objects_by_type(&obj_type);
    for obj in &objects {
        if let Some(result) = build_daily_counter_response(ctx, &date_str, obj.obj_hash, &counter) {
            dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
            dout.write_pack(&Pack::Map(MapPack { table: result }))?;
        }
    }
    Ok(())
}

/// COUNTER_TODAY_ALL: Get today's counter for ALL objects of a type.
pub fn counter_today_all(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };
    let obj_type = match table.get("objType") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let today = date::yyyymmdd_today();

    let objects = ctx.cache.object.get_live_objects_by_type(&obj_type);
    for obj in &objects {
        if let Some(result) = build_daily_counter_response(ctx, &today, obj.obj_hash, &counter) {
            dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
            dout.write_pack(&Pack::Map(MapPack { table: result }))?;
        }
    }
    Ok(())
}

/// COUNTER_PAST_DATE_GROUP: Get daily counter for a client-specified list of objHashes.
pub fn counter_past_date_group(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };
    let date_str = match table.get("date") {
        Some(Value::Text(s)) => s.clone(),
        _ => date::yyyymmdd_today(),
    };
    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let obj_hashes: Vec<i32> = match table.get("objHash") {
        Some(Value::List(list)) => list.iter().filter_map(|v| match v {
            Value::Decimal(d) => Some(*d as i32),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    };

    for obj_hash in &obj_hashes {
        if let Some(result) = build_daily_counter_response(ctx, &date_str, *obj_hash, &counter) {
            dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
            dout.write_pack(&Pack::Map(MapPack { table: result }))?;
        }
    }
    Ok(())
}

/// COUNTER_TODAY_GROUP: Get today's counter for a client-specified list of objHashes.
pub fn counter_today_group(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };
    let counter = match table.get("counter") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let obj_hashes: Vec<i32> = match table.get("objHash") {
        Some(Value::List(list)) => list.iter().filter_map(|v| match v {
            Value::Decimal(d) => Some(*d as i32),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    };
    let today = date::yyyymmdd_today();

    for obj_hash in &obj_hashes {
        if let Some(result) = build_daily_counter_response(ctx, &today, *obj_hash, &counter) {
            dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
            dout.write_pack(&Pack::Map(MapPack { table: result }))?;
        }
    }
    Ok(())
}
