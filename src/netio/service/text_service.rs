use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;
use crate::util::{date, hash};

/// Retrieve text by type and hash (with DB fallback).
pub fn get_text(
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

    let xtype = match table.get("type") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let text_hash = match table.get("hash") {
        Some(Value::Decimal(v)) => *v as i32,
        _ => 0,
    };

    let mut result = std::collections::HashMap::new();

    // Try cache first
    if let Some(text) = ctx.cache.text.get(&xtype, text_hash) {
        result.insert("text".into(), Value::Text(text));
    } else {
        // Fallback to DB
        let today = date::yyyymmdd_today();
        let div = hash::hash(&xtype);
        let found = if let Ok(container) = ctx.db_manager.get_or_create(&today) {
            container.text.get(div, text_hash).ok().flatten()
        } else {
            None
        };
        result.insert("text".into(), Value::Text(found.unwrap_or_default()));
    }

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    Ok(())
}

/// GET_TEXT_100: Batch text lookup, sending results in chunks of 100.
pub fn get_text_100(
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
    let xtype = match table.get("type") {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    };
    let hashes: Vec<i32> = match table.get("hash") {
        Some(Value::List(list)) => list.iter().filter_map(|v| match v {
            Value::Decimal(d) => Some(*d as i32),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    };

    let div = hash::hash(&xtype);
    let container = ctx.db_manager.get_or_create(&date_str).ok();

    let mut result_table = std::collections::HashMap::new();
    let mut count = 0;

    for text_hash in &hashes {
        // Try cache first
        let text = if let Some(t) = ctx.cache.text.get(&xtype, *text_hash) {
            Some(t)
        } else if let Some(ref c) = container {
            // Fallback to DB
            c.text.get(div, *text_hash).ok().flatten()
        } else {
            None
        };

        if let Some(t) = text {
            let key = hash::to_hexa32(*text_hash);
            result_table.insert(key, Value::Text(t));
            count += 1;

            // Flush every 100 entries
            if count >= 100 {
                dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
                dout.write_pack(&Pack::Map(MapPack { table: result_table }))?;
                result_table = std::collections::HashMap::new();
                count = 0;
            }
        }
    }

    // Send remaining entries
    if !result_table.is_empty() {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table: result_table }))?;
    }

    Ok(())
}
