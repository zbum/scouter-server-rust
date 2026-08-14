use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::routing::post;
use tower_http::cors::{CorsLayer, Any};
use tower_http::compression::CompressionLayer;

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
        .route("/telegraf/{obj_name}", post(handlers::telegraf::handle_telegraf))
        .layer(cors)
        .layer(CompressionLayer::new())
        .with_state(state)
}

/// Start the HTTP server. Called from main.rs.
pub async fn start_http_server(config: Arc<Config>, dispatcher: Arc<Dispatcher>) -> anyhow::Result<()> {
    if !config.net_http_server_enabled {
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
    axum::serve(listener, router).await?;
    Ok(())
}
