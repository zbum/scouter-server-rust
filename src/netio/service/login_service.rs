use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::netio::service::handler_registry::{ServiceContext, TcpReader};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;
use crate::protocol::value::Value;

pub fn login(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let mut table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };

    let id = get_text(&table, "id");
    let pass = get_text(&table, "pass");
    let ip = get_text(&table, "ip");
    let hostname = get_text(&table, "hostname");
    let client_ver = get_text(&table, "version");
    let internal = get_text(&table, "internal");
    let internal_mode = internal.eq_ignore_ascii_case("true");

    let session = ctx.login_manager.login(&id, &pass, &ip, internal_mode);

    table.insert("session".into(), Value::Decimal(session));

    if session == 0 {
        table.insert("error".into(), Value::Text("login fail".into()));
    } else {
        ctx.login_manager.update_user(session, |user| {
            user.hostname = hostname.clone();
            user.version = client_ver.clone();
        });

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        table.insert("time".into(), Value::Decimal(now));
        table.insert("server_id".into(), Value::Text(ctx.config.server_id.clone()));
        table.insert("type".into(), Value::Text(String::new()));
        table.insert("version".into(), Value::Text("Scouter Server Rust 0.1.0".into()));
    }

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn get_login_list(
    ctx: &ServiceContext,
    _din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let users = ctx.login_manager.get_login_user_list();
    if !users.is_empty() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let mut result = std::collections::HashMap::new();
        let mut sessions = Vec::new();
        let mut user_ids = Vec::new();
        let mut ips = Vec::new();
        let mut login_times = Vec::new();
        let mut versions = Vec::new();
        let mut hostnames = Vec::new();

        for usr in &users {
            sessions.push(Value::Decimal(usr.session));
            user_ids.push(Value::Text(usr.id.clone()));
            ips.push(Value::Text(usr.ip.clone()));
            login_times.push(Value::Decimal((now - usr.logintime) / 1000));
            versions.push(Value::Text(usr.version.clone()));
            hostnames.push(Value::Text(usr.hostname.clone()));
        }

        result.insert("session".into(), Value::List(sessions));
        result.insert("user".into(), Value::List(user_ids));
        result.insert("ip".into(), Value::List(ips));
        result.insert("logintime".into(), Value::List(login_times));
        result.insert("ver".into(), Value::List(versions));
        result.insert("host".into(), Value::List(hostnames));

        dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
        dout.write_pack(&Pack::Map(MapPack { table: result }))?;
    }
    Ok(())
}

pub fn check_session(
    ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    let pack = din.read_pack()?;
    let mut table = match pack {
        Pack::Map(m) => m.table,
        _ => std::collections::HashMap::new(),
    };

    let session = match table.get("session") {
        Some(Value::Decimal(v)) => *v,
        _ => 0,
    };

    let valid = ctx.login_manager.valid_session(session);
    table.insert("validSession".into(), Value::Decimal(valid));
    if valid == 0 {
        table.insert("error".into(), Value::Text("login fail".into()));
    }

    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_pack(&Pack::Map(MapPack { table }))?;
    Ok(())
}

pub fn check_login(
    _ctx: &ServiceContext,
    din: &mut DataInputX<TcpReader>,
    dout: &mut DataOutputX,
    _login: bool,
) -> Result<()> {
    // Read and discard the pack
    let _pack = din.read_pack()?;

    // Accept all logins for now
    dout.write_byte(tcp_flag::HAS_NEXT as i32)?;
    dout.write_value(&Value::Boolean(true))?;
    Ok(())
}

fn get_text(table: &std::collections::HashMap<String, Value>, key: &str) -> String {
    match table.get(key) {
        Some(Value::Text(s)) => s.clone(),
        _ => String::new(),
    }
}
