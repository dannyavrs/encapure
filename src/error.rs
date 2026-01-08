use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum AppError {
    #[error("Model inference failed: {0}")]
    ModelError(String),

    #[error("Invalid input: {0}")]
    ValidationError(String),

    #[error("Service temporarily unavailable: {0}")]
    ResourceError(String),

    #[error("Tokenization failed: {0}")]
    TokenizationError(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: u16,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::ModelError(e) => {
                tracing::error!(error = %e, "Model inference error");
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            AppError::ValidationError(msg) => {
                tracing::warn!(error = %msg, "Validation error");
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::ResourceError(msg) => {
                tracing::warn!(error = %msg, "Resource error");
                (StatusCode::SERVICE_UNAVAILABLE, msg.clone())
            }
            AppError::TokenizationError(msg) => {
                tracing::error!(error = %msg, "Tokenization error");
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
        };

        let body = Json(ErrorResponse {
            error: message,
            code: status.as_u16(),
        });

        (status, body).into_response()
    }
}

impl From<ort::Error> for AppError {
    fn from(err: ort::Error) -> Self {
        AppError::ModelError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
