mod config;
mod error;
mod handlers;
mod inference;
mod state;

use crate::config::Config;
use crate::handlers::{health_handler, ready_handler, rerank_handler};
use crate::state::AppState;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "encapure=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Encapure reranking service");

    // Load configuration
    let config = Config::from_env()?;
    let shutdown_timeout = config.shutdown_timeout_secs;
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    // Set up Prometheus metrics recorder
    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("Failed to install Prometheus recorder: {}", e))?;

    // Initialize application state (loads model and tokenizer)
    let start = std::time::Instant::now();
    let state = AppState::new(config)?;
    let state = Arc::new(state);
    tracing::info!(
        elapsed_ms = start.elapsed().as_millis() as u64,
        "State initialized",
    );

    // Build router
    let app = Router::new()
        // Core endpoints - rerank needs larger body limit for batch requests
        .route(
            "/rerank",
            post(rerank_handler).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        // Health endpoints
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        // Metrics endpoint
        .route(
            "/metrics",
            get(move || {
                let handle = prometheus_handle.clone();
                async move { handle.render() }
            }),
        )
        // Middleware
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        // State
        .with_state(state);

    // Create TCP listener
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(address = %addr, "Server listening");

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_timeout))
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM).
/// After signal, allows `timeout_secs` for in-flight requests to complete.
async fn shutdown_signal(timeout_secs: u64) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown");
        }
    }

    // Give in-flight requests time to complete
    tracing::info!(timeout_secs, "Draining connections...");
    tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
}
