use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::data_output;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;
use crate::util::date;

/// ALERT_REAL_TIME: Get real-time alerts since a given index (incremental polling).
pub fn alert_real_time(
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

    let index = match table.get("index") {
        Some(Value::Decimal(v)) => *v as u64,
        _ => 0,
    };
    let first = match table.get("first") {
        Some(Value::Boolean(v)) => *v,
        _ => false,
    };

    let (entries, current_loop, current_index) = ctx.cache.alert.get_since(index);

    // Send meta pack first
    let mut meta = std::collections::HashMap::new();
    meta.insert("loop".into(), Value::Decimal(current_loop as i64));
    meta.insert("index".into(), Value::Decimal(current_index as i64));
    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table: meta }))?;

    // Send alert data (skip on first poll)
    if !first {
        for entry in &entries {
            let mut alert_out = data_output::DataOutputX::new();
            if alert_out.write_pack(&Pack::Alert(entry.pack.clone())).is_ok() {
                dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
                dout.write_raw(alert_out.as_bytes())?;
            }
        }
    }

    Ok(())
}

/// ALERT_LOAD_TIME: Load historical alerts by time range from DB.
pub fn alert_load_time(
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
    let stime = match table.get("stime") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };
    let etime = match table.get("etime") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };
    let max_count = match table.get("count") {
        Some(Value::Decimal(v)) => *v as usize,
        _ => 500,
    };

    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        let mut count = 0;
        container.alert.read_by_time(stime, etime, &mut |_time: i64, data: Vec<u8>| {
            if count >= max_count {
                return;
            }
            if dout.write_byte(tcp_flag::HAS_NEXT as i32).is_ok() {
                let _ = dout.write_raw(&data);
            }
            count += 1;
        })?;
    }

    Ok(())
}
