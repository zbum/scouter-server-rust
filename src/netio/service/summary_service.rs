use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack, SUMMARY_APP, SUMMARY_SQL, SUMMARY_APICALL};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;
use crate::util::date;

fn load_summary(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    stype: u8,
) -> Result<()> {
    let pack = din.read_pack()?;
    let date_str = match &pack {
        Pack::Map(m) => {
            match m.table.get("date") {
                Some(Value::Text(s)) => s.clone(),
                Some(Value::Decimal(v)) => v.to_string(),
                _ => date::yyyymmdd_today(),
            }
        }
        _ => date::yyyymmdd_today(),
    };

    if let Ok(container) = ctx.db_manager.get_or_create(&date_str) {
        container.summary.read_by_type(stype, |key, data| {
            // key = hhmm(4B) + id_hash(4B)
            let hhmm = if key.len() >= 4 {
                i32::from_be_bytes(key[..4].try_into().unwrap_or([0; 4]))
            } else {
                0
            };
            let id_hash = if key.len() >= 8 {
                i32::from_be_bytes(key[4..8].try_into().unwrap_or([0; 4]))
            } else {
                0
            };

            // Deserialize the stored value
            let mut d = DataInputX::from_bytes(data);
            let value = d.read_value().ok();

            let mut table = std::collections::HashMap::new();
            table.insert("id".into(), Value::Decimal(id_hash as i64));
            table.insert("hhmm".into(), Value::Decimal(hhmm as i64));
            if let Some(v) = value {
                table.insert("data".into(), v);
            }

            let _ = dout.write_byte(tcp_flag::HAS_NEXT as i32);
            let _ = dout.write_pack(&Pack::Map(MapPack { table }));
        })?;
    }

    Ok(())
}

pub fn load_service_summary(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    load_summary(ctx, din, dout, SUMMARY_APP)
}

pub fn load_sql_summary(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    load_summary(ctx, din, dout, SUMMARY_SQL)
}

pub fn load_apicall_summary(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    load_summary(ctx, din, dout, SUMMARY_APICALL)
}
