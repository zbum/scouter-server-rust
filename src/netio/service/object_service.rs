use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

/// Returns all objects (agents) currently registered.
pub fn object_list_real_time(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let objects = ctx.cache.object.get_all_objects();

    for obj in objects {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Object(obj))?;
    }
    Ok(())
}

/// Returns objects for a historical date.
/// MVP: Returns all currently known objects (date-based DB lookup deferred to Phase 6).
pub fn object_list_load_date(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    // For now, return the same list as real-time (all known objects)
    let objects = ctx.cache.object.get_all_objects();

    for obj in objects {
        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Object(obj))?;
    }
    Ok(())
}

/// Returns info for a specific object by hash.
pub fn object_info(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let obj_hash = match pack {
        Pack::Map(ref m) => match m.table.get("objHash") {
            Some(Value::Decimal(v)) => *v as i32,
            _ => 0,
        },
        _ => 0,
    };

    if let Some(obj) = ctx.cache.object.get(obj_hash) {
        let mut table = std::collections::HashMap::new();
        table.insert("objHash".into(), Value::Decimal(obj.obj_hash as i64));
        table.insert("objName".into(), Value::Text(obj.obj_name.clone()));
        table.insert("objType".into(), Value::Text(obj.obj_type.clone()));
        table.insert("address".into(), Value::Text(obj.address.clone()));
        table.insert("alive".into(), Value::Boolean(obj.alive));
        table.insert("wakeup".into(), Value::Decimal(obj.wakeup));

        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table }))?;
    }
    Ok(())
}
