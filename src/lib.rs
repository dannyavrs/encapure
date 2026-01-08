//! Encapure - High-performance reranking microservice
//!
//! This library exposes the core components for the reranking service,
//! enabling integration tests and potential embedding in other applications.

pub mod config;
pub mod error;
pub mod handlers;
pub mod inference;
pub mod state;

// Re-export key types for convenience
pub use config::Config;
pub use error::{AppError, Result};
pub use handlers::{health_handler, ready_handler, rerank_handler};
pub use inference::{RerankerModel, TokenizerWrapper};
pub use state::AppState;
