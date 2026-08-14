use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

pub fn get_global_kv(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let key = match &pack {
        Pack::Map(m) => {
            match m.table.get("key") {
                Some(Value::Text(s)) => s.clone(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    };

    let mut table = std::collections::HashMap::new();
    if let Some(value) = ctx.kv_store.get(&key) {
        table.insert("value".into(), Value::Text(value));
    }

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn set_global_kv(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    if let Pack::Map(m) = &pack {
        let key = match m.table.get("key") {
            Some(Value::Text(s)) => s.clone(),
            _ => String::new(),
        };
        let value = match m.table.get("value") {
            Some(Value::Text(s)) => s.clone(),
            _ => String::new(),
        };

        if !key.is_empty() {
            ctx.kv_store.set(&key, &value);
        }
    }

    let mut table = std::collections::HashMap::new();
    table.insert("result".into(), Value::Boolean(true));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}
