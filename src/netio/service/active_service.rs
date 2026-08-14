use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

pub fn activespeed_real_time(
    _ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    // Read input pack
    let _pack = din.read_pack()?;

    // MVP: return empty active speed list
    let mut table = std::collections::HashMap::new();
    table.insert("act1".into(), Value::Decimal(0));
    table.insert("act2".into(), Value::Decimal(0));
    table.insert("act3".into(), Value::Decimal(0));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}
