use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use tracing::warn;

use crate::config::Config;
use crate::core::cache::CacheManager;
use crate::core::kv_store::KvStore;
use crate::db::db_manager::DbManager;
use crate::login::LoginManager;
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::request_cmd;

/// Context passed to every service handler.
pub struct ServiceContext {
    pub config: Arc<Config>,
    pub cache: Arc<CacheManager>,
    pub login_manager: Arc<LoginManager>,
    pub db_manager: Arc<DbManager>,
    pub kv_store: Arc<KvStore>,
}

/// Type alias for the reader used in TCP service handlers.
/// Handlers read directly from the TCP stream via this reader.
pub type TcpReader = Box<dyn io::Read + Send>;

/// Handler function signature: (context, input_reader, output_buffer, is_logged_in)
pub type HandlerFn = fn(
    &ServiceContext,
    &mut DataInputX<TcpReader>,
    &mut DataOutputX,
    bool,
) -> crate::error::Result<()>;

/// Registry mapping command strings to handler functions.
pub struct HandlerRegistry {
    handlers: HashMap<&'static str, HandlerFn>,
    pub context: Arc<ServiceContext>,
}

impl HandlerRegistry {
    pub fn new(
        config: Arc<Config>,
        cache: Arc<CacheManager>,
        login_manager: Arc<LoginManager>,
        db_manager: Arc<DbManager>,
        kv_store: Arc<KvStore>,
    ) -> Self {
        let context = Arc::new(ServiceContext {
            config,
            cache,
            login_manager,
            db_manager,
            kv_store,
        });

        let mut handlers: HashMap<&'static str, HandlerFn> = HashMap::new();

        // Login
        handlers.insert(request_cmd::LOGIN, super::login_service::login);
        handlers.insert(
            request_cmd::GET_LOGIN_LIST,
            super::login_service::get_login_list,
        );
        handlers.insert(
            request_cmd::CHECK_SESSION,
            super::login_service::check_session,
        );
        handlers.insert(request_cmd::CHECK_LOGIN, super::login_service::check_login);

        // Server
        handlers.insert(
            request_cmd::SERVER_VERSION,
            super::server_service::server_version,
        );
        handlers.insert(request_cmd::SERVER_TIME, super::server_service::server_time);
        handlers.insert(
            request_cmd::SERVER_STATUS,
            super::server_service::server_status,
        );
        handlers.insert(request_cmd::CHECK_JOB, super::server_service::check_job);

        // Object
        handlers.insert(
            request_cmd::OBJECT_LIST_REAL_TIME,
            super::object_service::object_list_real_time,
        );
        handlers.insert(request_cmd::OBJECT_INFO, super::object_service::object_info);

        // Counter
        handlers.insert(
            request_cmd::COUNTER_REAL_TIME,
            super::counter_service::counter_real_time,
        );
        handlers.insert(
            request_cmd::COUNTER_REAL_TIME_ALL,
            super::counter_service::counter_real_time_all,
        );
        handlers.insert(
            request_cmd::COUNTER_REAL_TIME_MULTI,
            super::counter_service::counter_real_time_multi,
        );
        handlers.insert(
            request_cmd::COUNTER_PAST_TIME,
            super::counter_service::counter_past_time,
        );

        // XLog
        handlers.insert(
            request_cmd::TRANX_REAL_TIME_GROUP,
            super::xlog_service::xlog_real_time,
        );
        handlers.insert(
            request_cmd::TRANX_REAL_TIME_GROUP_LATEST,
            super::xlog_service::xlog_real_time_latest,
        );

        // Text
        handlers.insert(request_cmd::GET_TEXT, super::text_service::get_text);
        handlers.insert(request_cmd::GET_TEXT_100, super::text_service::get_text_100);

        // Counter historical
        handlers.insert(
            request_cmd::COUNTER_PAST_DATE,
            super::counter_service::counter_past_date,
        );
        handlers.insert(
            request_cmd::COUNTER_TODAY,
            super::counter_service::counter_today,
        );
        handlers.insert(
            request_cmd::COUNTER_PAST_DATE_ALL,
            super::counter_service::counter_past_date_all,
        );
        handlers.insert(
            request_cmd::COUNTER_TODAY_ALL,
            super::counter_service::counter_today_all,
        );
        handlers.insert(
            request_cmd::COUNTER_PAST_DATE_GROUP,
            super::counter_service::counter_past_date_group,
        );
        handlers.insert(
            request_cmd::COUNTER_TODAY_GROUP,
            super::counter_service::counter_today_group,
        );

        // XLog historical
        handlers.insert(
            request_cmd::TRANX_LOAD_TIME_GROUP,
            super::xlog_service::xlog_load_time_group,
        );
        handlers.insert(
            request_cmd::TRANX_PROFILE,
            super::xlog_service::xlog_profile,
        );
        handlers.insert(
            request_cmd::XLOG_READ_BY_TXID,
            super::xlog_service::xlog_read_by_txid,
        );
        handlers.insert(
            request_cmd::XLOG_READ_BY_GXID,
            super::xlog_service::xlog_read_by_gxid,
        );
        handlers.insert(
            request_cmd::XLOG_LOAD_BY_GXID,
            super::xlog_service::xlog_load_by_gxid,
        );

        // Alert
        handlers.insert(
            request_cmd::ALERT_REAL_TIME,
            super::alert_service::alert_real_time,
        );
        handlers.insert(
            request_cmd::ALERT_LOAD_TIME,
            super::alert_service::alert_load_time,
        );

        // Object historical
        handlers.insert(
            request_cmd::OBJECT_LIST_LOAD_DATE,
            super::object_service::object_list_load_date,
        );

        // Summary
        handlers.insert(
            request_cmd::LOAD_SERVICE_SUMMARY,
            super::summary_service::load_service_summary,
        );
        handlers.insert(
            request_cmd::LOAD_SQL_SUMMARY,
            super::summary_service::load_sql_summary,
        );
        handlers.insert(
            request_cmd::LOAD_APICALL_SUMMARY,
            super::summary_service::load_apicall_summary,
        );

        // Config
        handlers.insert(
            request_cmd::GET_CONFIGURE_SERVER,
            super::config_service::get_configure_server,
        );
        handlers.insert(
            request_cmd::SET_CONFIGURE_SERVER,
            super::config_service::set_configure_server,
        );
        handlers.insert(
            request_cmd::LIST_CONFIGURE_SERVER,
            super::config_service::list_configure_server,
        );
        handlers.insert(
            request_cmd::GET_XML_COUNTER,
            super::config_service::get_xml_counter,
        );

        // KV Store
        handlers.insert(request_cmd::GET_GLOBAL_KV, super::kv_service::get_global_kv);
        handlers.insert(request_cmd::SET_GLOBAL_KV, super::kv_service::set_global_kv);

        // Status
        handlers.insert(
            request_cmd::STATUS_AROUND_VALUE,
            super::status_service::status_around_value,
        );

        // Active Speed
        handlers.insert(
            request_cmd::ACTIVESPEED_REAL_TIME,
            super::active_service::activespeed_real_time,
        );

        // Server extended
        handlers.insert(
            request_cmd::SERVER_DB_LIST,
            super::server_service::server_db_list,
        );
        handlers.insert(request_cmd::SERVER_ENV, super::server_service::server_env);

        Self { handlers, context }
    }

    /// Process a command. Returns true if a handler was found.
    pub fn process(
        &self,
        cmd: &str,
        din: &mut DataInputX<TcpReader>,
        dout: &mut DataOutputX,
        login: bool,
    ) -> bool {
        if let Some(handler) = self.handlers.get(cmd) {
            if let Err(e) = handler(&self.context, din, dout, login) {
                warn!("Handler error for {}: {}", cmd, e);
            }
            true
        } else {
            warn!("Unknown command: {}", cmd);
            false
        }
    }
}
