use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

use scouter_server_rust::config::Config;
use scouter_server_rust::core::agent_manager::AgentManager;
use scouter_server_rust::core::alert_core::AlertCore;
use scouter_server_rust::core::alert_summary::AlertSummary;
use scouter_server_rust::core::cache::CacheManager;
use scouter_server_rust::core::dispatcher::Dispatcher;
use scouter_server_rust::core::kv_store::KvStore;
use scouter_server_rust::core::perf_count_core::PerfCountCore;
use scouter_server_rust::core::profile_core::ProfileCore;
use scouter_server_rust::core::status_core::StatusCore;
use scouter_server_rust::core::summary_core::SummaryCore;
use scouter_server_rust::core::text_core::TextCore;
use scouter_server_rust::core::xlog_core::XLogCore;
use scouter_server_rust::login::LoginManager;
use scouter_server_rust::netio::net_data_processor::NetDataProcessor;
use scouter_server_rust::netio::service::handler_registry::HandlerRegistry;
use scouter_server_rust::netio::tcp::tcp_agent_manager::TcpAgentManager;
use scouter_server_rust::netio::tcp::tcp_server;
use scouter_server_rust::db::cleanup::Cleaner;
use scouter_server_rust::db::db_manager::DbManager;
use scouter_server_rust::http;
use scouter_server_rust::netio::udp_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    print_logo();

    // Load config
    let config = Arc::new(Config::load("conf/scouter.toml"));
    info!("Server ID: {}", config.server_id);
    info!(
        "UDP listen: {}:{}",
        config.net_udp_listen_ip, config.net_udp_listen_port
    );
    info!(
        "TCP listen: {}:{}",
        config.net_tcp_listen_ip, config.net_tcp_listen_port
    );

    // Shutdown token
    let shutdown_token = CancellationToken::new();

    // Initialize cache
    let cache = Arc::new(CacheManager::new(&config));
    info!("Cache manager initialized");

    // Initialize login manager
    let login_manager = Arc::new(LoginManager::new());
    login_manager.clone().start_cleaner();
    info!("Login manager initialized");

    // Initialize DB manager
    let db_manager = Arc::new(DbManager::new(config.clone()));
    db_manager.clone().start_monitor();
    Cleaner::start(config.db_dir.clone(), config.db_keep_days);
    info!("DB manager initialized (dir: {})", config.db_dir);

    // Initialize KV store
    let kv_store = Arc::new(KvStore::new());

    // Initialize core processors (with shutdown token)
    let agent_manager = Arc::new(AgentManager::new(config.clone(), cache.clone()));
    let perf_count_core = Arc::new(PerfCountCore::new(cache.clone(), db_manager.clone(), config.counter_queue_size, shutdown_token.clone()));
    let xlog_core = Arc::new(XLogCore::new(cache.clone(), db_manager.clone(), config.xlog_queue_size, shutdown_token.clone()));
    let text_core = Arc::new(TextCore::new(cache.clone(), db_manager.clone(), config.text_queue_size, shutdown_token.clone()));
    let summary_core = Arc::new(SummaryCore::new(db_manager.clone(), config.summary_queue_size, shutdown_token.clone()));
    let alert_summary = Arc::new(AlertSummary::new(summary_core.clone()));
    let alert_core = Arc::new(AlertCore::new(cache.clone(), db_manager.clone(), config.alert_queue_size, shutdown_token.clone(), alert_summary));
    let profile_core = Arc::new(ProfileCore::new(db_manager.clone(), config.profile_queue_size, shutdown_token.clone()));
    let status_core = Arc::new(StatusCore::new(cache.clone(), config.status_queue_size, shutdown_token.clone()));
    info!("Core processors initialized");

    // Start agent monitor daemon
    agent_manager.clone().start_monitor();
    info!("Agent monitor started");

    // Create dispatcher
    let dispatcher = Arc::new(Dispatcher::new(
        config.clone(),
        agent_manager,
        perf_count_core,
        xlog_core,
        text_core,
        alert_core,
        profile_core,
        status_core,
        summary_core,
    ));

    // Clone dispatcher for HTTP before moving into NetDataProcessor
    let http_dispatcher = dispatcher.clone();

    // Create UDP processor channel and start
    let (tx, rx) = mpsc::channel(2048);
    let processor = Arc::new(NetDataProcessor::new(config.clone(), dispatcher));
    processor.start(rx);

    // Start UDP server
    let udp_config = config.clone();
    let udp_handle = tokio::spawn(async move {
        if let Err(e) = udp_server::start_udp_server(udp_config, tx).await {
            tracing::error!("UDP server error: {}", e);
        }
    });

    // Initialize TCP components
    let tcp_agent_manager = Arc::new(TcpAgentManager::new());
    tcp_agent_manager.clone().start_monitor();

    let handler_registry = Arc::new(HandlerRegistry::new(
        config.clone(),
        cache.clone(),
        login_manager,
        db_manager.clone(),
        kv_store,
    ));

    // Start TCP server
    let tcp_config = config.clone();
    let tcp_handle = tokio::spawn(async move {
        if let Err(e) = tcp_server::start_tcp_server(tcp_config, handler_registry, tcp_agent_manager).await {
            tracing::error!("TCP server error: {}", e);
        }
    });

    // Start HTTP server
    let http_config = config.clone();
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http::start_http_server(http_config, http_dispatcher).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    info!("Scouter Server (Rust) started successfully");
    info!("Waiting for agent connections...");

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal, initiating graceful shutdown...");
        }
        _ = udp_handle => {
            info!("UDP server stopped");
        }
        _ = tcp_handle => {
            info!("TCP server stopped");
        }
        _ = http_handle => {
            info!("HTTP server stopped");
        }
    }

    // Graceful shutdown sequence
    info!("Cancelling core workers...");
    shutdown_token.cancel();

    // Give workers time to drain their queues (up to 5 seconds)
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Flush all DB containers
    info!("Flushing DB...");
    db_manager.flush_all();

    info!("Shutdown complete");
    Ok(())
}

fn print_logo() {
    println!(
        r#"
  ____                  _              ____
 / ___|  ___ ___  _   _| |_ ___ _ __ / ___|  ___ _ ____   _____ _ __
 \___ \ / __/ _ \| | | | __/ _ \ '__| \___ \ / _ \ '__\ \ / / _ \ '__|
  ___) | (_| (_) | |_| | ||  __/ |    ___) |  __/ |   \ V /  __/ |
 |____/ \___\___/ \__,_|\__\___|_|   |____/ \___|_|    \_/ \___|_|
                                                          [Rust Edition]
"#
    );
}
