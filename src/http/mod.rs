use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::post;
use axum::Router;
use tokio_util::sync::CancellationToken;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};

use crate::config::Config;
use crate::core::dispatcher::Dispatcher;

pub mod error;
pub mod handlers;
pub mod models;

/// Shared state accessible by all HTTP handlers via Axum's State extractor.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub dispatcher: Arc<Dispatcher>,
}

/// Build the Axum router with all routes and middleware.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/counter", post(handlers::counter::handle_counter))
        .route("/register", post(handlers::register::handle_register))
        .route(
            "/telegraf/{obj_name}",
            post(handlers::telegraf::handle_telegraf),
        )
        .layer(cors)
        .layer(CompressionLayer::new())
        .with_state(state)
}

/// Start the HTTP server. Called from main.rs.
pub async fn start_http_server(
    config: Arc<Config>,
    dispatcher: Arc<Dispatcher>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    if wait_if_http_disabled(config.net_http_server_enabled, &shutdown).await {
        tracing::info!("HTTP server disabled by config");
        return Ok(());
    }

    let state = AppState {
        config: config.clone(),
        dispatcher,
    };

    let router = build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.net_http_port));
    tracing::info!("HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;
    Ok(())
}

async fn wait_if_http_disabled(enabled: bool, shutdown: &CancellationToken) -> bool {
    if enabled {
        return false;
    }

    shutdown.cancelled().await;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn disabled_http_waits_for_shutdown() {
        let shutdown = CancellationToken::new();
        let wait = wait_if_http_disabled(false, &shutdown);
        tokio::pin!(wait);

        assert!(tokio::time::timeout(Duration::from_millis(20), &mut wait)
            .await
            .is_err());

        shutdown.cancel();
        assert!(tokio::time::timeout(Duration::from_secs(1), &mut wait)
            .await
            .expect("disabled HTTP task should stop after cancellation"));
    }
}
