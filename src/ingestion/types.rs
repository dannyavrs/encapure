//! Type definitions for the ingestion module.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A normalized tool record ready for semantic search indexing.
///
/// This struct represents an atomic tool unit parsed from an MCP server's
/// `list_tools` response. The `inference_view` field is pre-computed for
/// optimal reranker performance, while `raw_definition` preserves the full
/// schema for agent invocation.
///
/// # Design Rationale
/// - `inference_view`: Pre-computed text representation for the reranker model
/// - `raw_definition`: Preserved for agent runtime (schema validation, invocation)
/// - `server_origin`: Enables filtering by source MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncapureTool {
    /// Unique tool identifier (from MCP tool.name)
    pub name: String,

    /// The MCP server that provided this tool (e.g., "filesystem", "github")
    pub server_origin: String,

    /// Pre-formatted text for reranker embedding.
    /// Format: "TOOL: <name> | CONTEXT: <server_name> | FUNC: <description> | INPUTS: <param_summary>"
    pub inference_view: String,

    /// Full JSON schema preserved for agent invocation
    pub raw_definition: Value,
}

impl EncapureTool {
    /// Creates a new EncapureTool with the given parameters.
    pub fn new(
        name: String,
        server_origin: String,
        inference_view: String,
        raw_definition: Value,
    ) -> Self {
        Self {
            name,
            server_origin,
            inference_view,
            raw_definition,
        }
    }
}
