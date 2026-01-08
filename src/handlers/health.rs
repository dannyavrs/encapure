use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// GET /health - Liveness probe
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// GET /ready - Readiness probe (checks model is loaded and warmed up)
pub async fn ready_handler(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<HealthResponse>) {
    if state.is_ready() {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ready",
                version: env!("CARGO_PKG_VERSION"),
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "not_ready",
                version: env!("CARGO_PKG_VERSION"),
            }),
        )
    }
}
