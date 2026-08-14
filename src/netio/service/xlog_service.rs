use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;
use crate::util::date;

/// Get real-time XLog data since a given index.
pub fn xlog_real_time(
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

    let entries = ctx.cache.xlog.get_since(index, None);
    let new_index = ctx.cache.xlog.latest_index();

    // Send each xlog as raw bytes
    for entry in entries {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_raw(&entry.data)?;
    }

    // Send the new loop index
    let mut meta = std::collections::HashMap::new();
    meta.insert("index".into(), Value::Decimal(new_index as i64));
    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table: meta }))?;

    Ok(())
}

/// Get latest count of XLogs.
pub fn xlog_real_time_latest(
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

    let count = match table.get("count") {
        Some(Value::Decimal(v)) => *v as usize,
        _ => 100,
    };

    let entries = ctx.cache.xlog.get_latest(count);
    let new_index = ctx.cache.xlog.latest_index();

    for entry in entries {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_raw(&entry.data)?;
    }

    let mut meta = std::collections::HashMap::new();
    meta.insert("index".into(), Value::Decimal(new_index as i64));
    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table: meta }))?;

    Ok(())
}

// ─── Historical XLog handlers ───

/// TRANX_LOAD_TIME_GROUP: Load historical XLogs by time range.
pub fn xlog_load_time_group(
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
    let max = match table.get("max") {
        Some(Value::Decimal(v)) => *v as usize,
        _ => 500,
    };

    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        let mut count = 0;
        container.xlog.read_by_time(stime, etime, &mut |_time: i64, data: Vec<u8>| {
            if count >= max {
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

/// TRANX_PROFILE: Get profile data for a specific XLog by txid.
pub fn xlog_profile(
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
    let txid = match table.get("txid") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };

    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        if let Ok(Some(data)) = container.profile.read_by_txid(txid) {
            dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
            dout.write_raw(&data)?;
        }
    }

    Ok(())
}

/// XLOG_READ_BY_TXID: Get a single XLog by transaction ID.
pub fn xlog_read_by_txid(
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
    let txid = match table.get("txid") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };

    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        if let Ok(Some(data)) = container.xlog.read_by_txid(txid) {
            dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
            dout.write_raw(&data)?;
        }
    }

    Ok(())
}

/// XLOG_READ_BY_GXID: Get all XLogs sharing a global transaction ID.
pub fn xlog_read_by_gxid(
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
    let gxid = match table.get("gxid") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };

    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        if let Ok(entries) = container.xlog.read_by_gxid(gxid) {
            for data in entries {
                dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
                dout.write_raw(&data)?;
            }
        }
    }

    Ok(())
}

/// XLOG_LOAD_BY_GXID: Load XLogs by gxid across two dates.
pub fn xlog_load_by_gxid(
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
    let gxid = match table.get("gxid") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };

    // Read from the primary date
    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        if let Ok(entries) = container.xlog.read_by_gxid(gxid) {
            for data in entries {
                dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
                dout.write_raw(&data)?;
            }
        }
    }

    // Also check adjacent date (previous day) for cross-date transactions
    let date_millis_val = date::date_millis(&date_str);
    if date_millis_val > 0 {
        let prev_date = date::yyyymmdd(date_millis_val - 86_400_000);
        if prev_date != date_str {
            if let Ok(container) = ctx.db_manager.get_or_create(&prev_date) {
                if let Ok(entries) = container.xlog.read_by_gxid(gxid) {
                    for data in entries {
                        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
                        dout.write_raw(&data)?;
                    }
                }
            }
        }
    }

    Ok(())
}
