use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

const DEFAULT_COUNTER_XML_PATH: &str =
    "../../scouter/scouter.common/src/main/resources/scouter/lang/counters/counters.xml";

pub fn get_xml_counter(
    _ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let path = std::env::var("SCOUTER_COUNTER_XML")
        .unwrap_or_else(|_| DEFAULT_COUNTER_XML_PATH.to_string());
    let default_xml = std::fs::read(&path).map_err(|error| {
        crate::error::ScouterError::Config(format!(
            "failed to read counter XML from {path}: {error}"
        ))
    })?;

    let mut table = std::collections::HashMap::new();
    table.insert("default".into(), Value::Blob(default_xml));
    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn get_configure_server(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    // Return server config as a text representation
    let config = &ctx.config;
    let text = format!(
        "server_id={}\nnet_udp_listen_port={}\nnet_tcp_listen_port={}\nnet_http_port={}\ndb_dir={}\ndb_keep_days={}",
        config.server_id, config.net_udp_listen_port, config.net_tcp_listen_port,
        config.net_http_port, config.db_dir, config.db_keep_days
    );

    let mut table = std::collections::HashMap::new();
    table.insert("config".into(), Value::Text(text));

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn set_configure_server(
    _ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    // Read and discard - config changes not supported at runtime in MVP
    let _pack = din.read_pack()?;

    let mut table = std::collections::HashMap::new();
    table.insert("result".into(), Value::Boolean(false));
    table.insert(
        "message".into(),
        Value::Text("Runtime config change not supported yet".into()),
    );

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn list_configure_server(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let config = &ctx.config;

    let mut table = std::collections::HashMap::new();
    table.insert("server_id".into(), Value::Text(config.server_id.clone()));
    table.insert(
        "net_udp_listen_port".into(),
        Value::Decimal(config.net_udp_listen_port as i64),
    );
    table.insert(
        "net_tcp_listen_port".into(),
        Value::Decimal(config.net_tcp_listen_port as i64),
    );
    table.insert(
        "net_http_port".into(),
        Value::Decimal(config.net_http_port as i64),
    );
    table.insert("db_dir".into(), Value::Text(config.db_dir.clone()));
    table.insert(
        "db_keep_days".into(),
        Value::Decimal(config.db_keep_days as i64),
    );
    table.insert(
        "xlog_queue_size".into(),
        Value::Decimal(config.xlog_queue_size as i64),
    );
    table.insert(
        "counter_queue_size".into(),
        Value::Decimal(config.counter_queue_size as i64),
    );

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}
