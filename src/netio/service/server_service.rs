use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

pub fn server_version(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let mut table = std::collections::HashMap::new();
    table.insert("version".into(), Value::Text("Scouter Server Rust 0.1.0".into()));
    table.insert("server_id".into(), Value::Text(ctx.config.server_id.clone()));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn server_time(
    _ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut table = std::collections::HashMap::new();
    table.insert("time".into(), Value::Decimal(now));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn server_status(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let mut table = std::collections::HashMap::new();
    table.insert("server_id".into(), Value::Text(ctx.config.server_id.clone()));
    table.insert("obj_count".into(), Value::Decimal(ctx.cache.object.size() as i64));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn server_db_list(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let db_dir = ctx.db_manager.db_dir();
    let mut dates: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(db_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        // Only include YYYYMMDD directories
                        if name.len() == 8 && name.chars().all(|c| c.is_ascii_digit()) {
                            dates.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    dates.sort_by(|a, b| b.cmp(a)); // newest first

    let list: Vec<Value> = dates.into_iter().map(Value::Text).collect();
    let mut table = std::collections::HashMap::new();
    table.insert("list".into(), Value::List(list));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn server_env(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let mut table = std::collections::HashMap::new();
    table.insert("os".into(), Value::Text(std::env::consts::OS.to_string()));
    table.insert("arch".into(), Value::Text(std::env::consts::ARCH.to_string()));
    table.insert("server_id".into(), Value::Text(ctx.config.server_id.clone()));
    table.insert("db_dir".into(), Value::Text(ctx.config.db_dir.clone()));
    table.insert("version".into(), Value::Text("Scouter Server Rust 0.1.0".into()));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}
