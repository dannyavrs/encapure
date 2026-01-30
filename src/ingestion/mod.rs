//! Ingestion module for processing external tool definitions.
//!
//! This module transforms MCP (Model Context Protocol) tool definitions
//! into structured records suitable for semantic search via the reranker.

pub mod atomizer;
pub mod types;

pub use atomizer::{atomize_tools, AtomizerResult};
pub use types::EncapureTool;
