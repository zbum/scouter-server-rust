use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

pub fn status_around_value(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let obj_hash = match &pack {
        Pack::Map(m) => {
            match m.table.get("objHash") {
                Some(Value::Decimal(v)) => *v as i32,
                _ => 0,
            }
        }
        _ => 0,
    };

    let statuses = ctx.cache.status.get_by_obj(obj_hash);
    for status in statuses {
        let mut table = std::collections::HashMap::new();
        table.insert("objHash".into(), Value::Decimal(obj_hash as i64));
        table.insert("key".into(), Value::Text(status.key.clone()));
        for (k, v) in &status.data.table {
            table.insert(k.clone(), v.clone());
        }

        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table }))?;
    }

    Ok(())
}
