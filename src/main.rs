use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
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
use scouter_server_rust::db::cleanup::Cleaner;
use scouter_server_rust::db::db_manager::DbManager;
use scouter_server_rust::http;
use scouter_server_rust::login::LoginManager;
use scouter_server_rust::netio::net_data_processor::NetDataProcessor;
use scouter_server_rust::netio::service::handler_registry::HandlerRegistry;
use scouter_server_rust::netio::tcp::tcp_agent_manager::TcpAgentManager;
use scouter_server_rust::netio::tcp::tcp_server;
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
    let core_shutdown_token = CancellationToken::new();
    let core_tasks = TaskTracker::new();
    let mut background_tasks = Vec::new();

    // Initialize cache
    let cache = Arc::new(CacheManager::new(&config));
    info!("Cache manager initialized");

    // Initialize login manager
    let login_manager = Arc::new(LoginManager::new());
    background_tasks.push(login_manager.clone().start_cleaner(shutdown_token.clone()));
    info!("Login manager initialized");

    // Initialize DB manager
    let db_manager = Arc::new(DbManager::new(config.clone()));
    background_tasks.push(db_manager.clone().start_monitor(shutdown_token.clone()));
    background_tasks.push(Cleaner::start(
        config.db_dir.clone(),
        config.db_keep_days,
        shutdown_token.clone(),
    ));
    info!("DB manager initialized (dir: {})", config.db_dir);

    // Initialize KV store
    let kv_store = Arc::new(KvStore::new());

    // Initialize core processors (with shutdown token)
    let agent_manager = Arc::new(AgentManager::new(config.clone(), cache.clone()));
    let perf_count_core = Arc::new(PerfCountCore::new(
        cache.clone(),
        db_manager.clone(),
        config.counter_queue_size,
        core_shutdown_token.clone(),
        &core_tasks,
    ));
    let xlog_core = Arc::new(XLogCore::new(
        cache.clone(),
        db_manager.clone(),
        config.xlog_queue_size,
        core_shutdown_token.clone(),
        &core_tasks,
    ));
    let text_core = Arc::new(TextCore::new(
        cache.clone(),
        db_manager.clone(),
        config.text_queue_size,
        core_shutdown_token.clone(),
        &core_tasks,
    ));
    let summary_core = Arc::new(SummaryCore::new(
        db_manager.clone(),
        config.summary_queue_size,
        core_shutdown_token.clone(),
        &core_tasks,
    ));
    let alert_summary = Arc::new(AlertSummary::new(summary_core.clone()));
    let alert_core = Arc::new(AlertCore::new(
        cache.clone(),
        db_manager.clone(),
        config.alert_queue_size,
        core_shutdown_token.clone(),
        alert_summary,
        &core_tasks,
    ));
    let profile_core = Arc::new(ProfileCore::new(
        db_manager.clone(),
        config.profile_queue_size,
        core_shutdown_token.clone(),
        &core_tasks,
    ));
    let status_core = Arc::new(StatusCore::new(
        cache.clone(),
        config.status_queue_size,
        core_shutdown_token.clone(),
        &core_tasks,
    ));
    info!("Core processors initialized");

    // Start agent monitor daemon
    background_tasks.push(agent_manager.clone().start_monitor(shutdown_token.clone()));
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
    background_tasks.push(processor.start(rx, shutdown_token.clone()));

    // Start UDP server
    let udp_config = config.clone();
    let udp_shutdown = shutdown_token.clone();

    // Initialize TCP components
    let tcp_agent_manager = Arc::new(TcpAgentManager::new());
    background_tasks.push(
        tcp_agent_manager
            .clone()
            .start_monitor(shutdown_token.clone()),
    );

    let handler_registry = Arc::new(HandlerRegistry::new(
        config.clone(),
        cache.clone(),
        login_manager,
        db_manager.clone(),
        kv_store,
    ));

    // Start TCP server
    let tcp_config = config.clone();
    let tcp_shutdown = shutdown_token.clone();

    // Start HTTP server
    let http_config = config.clone();
    let http_shutdown = shutdown_token.clone();

    let mut servers = tokio::task::JoinSet::new();
    servers.spawn(async move {
        (
            "UDP",
            udp_server::start_udp_server(udp_config, tx, udp_shutdown)
                .await
                .map_err(anyhow::Error::from),
        )
    });
    servers.spawn(async move {
        (
            "TCP",
            tcp_server::start_tcp_server(
                tcp_config,
                handler_registry,
                tcp_agent_manager,
                tcp_shutdown,
            )
            .await,
        )
    });
    servers.spawn(async move {
        (
            "HTTP",
            http::start_http_server(http_config, http_dispatcher, http_shutdown).await,
        )
    });

    info!("Scouter Server (Rust) started successfully");
    info!("Waiting for agent connections...");

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal, initiating graceful shutdown...");
        }
        result = servers.join_next() => {
            match result {
                Some(Ok((name, Ok(())))) => tracing::warn!("{} server stopped unexpectedly", name),
                Some(Ok((name, Err(e)))) => tracing::error!("{} server failed: {}", name, e),
                Some(Err(e)) => tracing::error!("Server task failed: {}", e),
                None => tracing::error!("All server tasks stopped unexpectedly"),
            }
        }
    }

    // Graceful shutdown sequence
    info!("Stopping listeners and input processors...");
    shutdown_token.cancel();

    while let Some(result) = servers.join_next().await {
        match result {
            Ok((name, Ok(()))) => info!("{} server stopped", name),
            Ok((name, Err(e))) => tracing::error!("{} server shutdown error: {}", name, e),
            Err(e) => tracing::error!("Server task join error: {}", e),
        }
    }

    for task in background_tasks {
        if let Err(e) = task.await {
            tracing::error!("Background task join error: {}", e);
        }
    }

    info!("Draining core workers...");
    core_tasks.close();
    core_shutdown_token.cancel();
    core_tasks.wait().await;

    // Flush all DB containers
    info!("Flushing DB...");
    if let Err(error) = db_manager.flush_all() {
        tracing::error!("Final DB flush failed: {}", error);
        return Err(error.into());
    }

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
