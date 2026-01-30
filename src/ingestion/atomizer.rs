//! Tool atomization logic for transforming MCP responses into searchable units.
//!
//! This module provides the core functionality to parse MCP `list_tools` JSON-RPC
//! responses and transform them into `EncapureTool` records optimized for
//! semantic search via the reranker.

use crate::error::AppError;
use crate::ingestion::types::EncapureTool;
use serde_json::Value;
use std::collections::HashSet;

/// Result type for atomizer operations
pub type AtomizerResult<T> = std::result::Result<T, AppError>;

/// Maximum description length before truncation
const MAX_DESCRIPTION_LENGTH: usize = 500;

/// Maximum parameter description length for the summary
const MAX_PARAM_DESC_LENGTH: usize = 50;

/// Transform an MCP list_tools JSON-RPC response into EncapureTool records.
///
/// # Arguments
/// * `json` - The full JSON-RPC response (must contain `result.tools` array)
/// * `server_name` - The origin MCP server name (used in inference_view)
///
/// # Errors
/// Returns `AppError::AtomizerError` if the JSON is not a valid MCP response.
/// Individual malformed tools are logged and skipped (partial success model).
///
/// # Example
/// ```ignore
/// let response = serde_json::json!({
///     "jsonrpc": "2.0",
///     "result": { "tools": [...] }
/// });
/// let tools = atomize_tools(&response, "filesystem")?;
/// ```
pub fn atomize_tools(json: &Value, server_name: &str) -> AtomizerResult<Vec<EncapureTool>> {
    // Navigate to result.tools array
    let tools_array = extract_tools_array(json)?;

    // Pre-allocate with capacity for efficiency
    let mut results = Vec::with_capacity(tools_array.len());

    // Process each tool with error tolerance
    for (idx, tool_value) in tools_array.iter().enumerate() {
        match normalize_tool(tool_value, server_name) {
            Ok(tool) => results.push(tool),
            Err(e) => {
                // Log and skip malformed tools (partial success model)
                tracing::warn!(
                    index = idx,
                    error = %e,
                    "Skipping malformed tool definition"
                );
            }
        }
    }

    if results.is_empty() && !tools_array.is_empty() {
        // All tools failed - likely a structural problem
        return Err(AppError::AtomizerError(
            "All tool definitions failed to parse".into(),
        ));
    }

    tracing::debug!(
        total = tools_array.len(),
        parsed = results.len(),
        server = server_name,
        "Tool atomization complete"
    );

    Ok(results)
}

/// Extract the tools array from an MCP JSON-RPC response.
///
/// Navigates the path: root -> result -> tools
fn extract_tools_array(json: &Value) -> AtomizerResult<&Vec<Value>> {
    json.get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .ok_or_else(|| {
            AppError::AtomizerError("Expected 'result.tools' array in MCP response".into())
        })
}

/// Transform a single tool definition into an EncapureTool.
fn normalize_tool(tool_value: &Value, server_name: &str) -> AtomizerResult<EncapureTool> {
    // Extract name (required field)
    let name = tool_value
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::AtomizerError(format!(
                "Tool missing required 'name' field: {:?}",
                tool_value.get("name")
            ))
        })?;

    // Extract description (optional, default to empty string)
    let description = tool_value
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Truncate long descriptions for the inference view
    let truncated_desc = truncate_description(description);

    // Build parameter summary from inputSchema
    let param_summary = build_param_summary(tool_value.get("inputSchema"));

    // Construct the inference view string
    let inference_view = build_inference_view(name, server_name, &truncated_desc, &param_summary);

    Ok(EncapureTool::new(
        name.to_string(),
        server_name.to_string(),
        inference_view,
        tool_value.clone(),
    ))
}

/// Truncate description to MAX_DESCRIPTION_LENGTH with ellipsis.
///
/// Attempts to truncate at a word boundary when possible.
fn truncate_description(desc: &str) -> String {
    if desc.len() <= MAX_DESCRIPTION_LENGTH {
        return desc.to_string();
    }

    let truncated = &desc[..MAX_DESCRIPTION_LENGTH];

    // Try to truncate at a word boundary
    match truncated.rfind(' ') {
        Some(pos) if pos > MAX_DESCRIPTION_LENGTH - 50 => {
            format!("{}...", &truncated[..pos])
        }
        _ => format!("{}...", truncated),
    }
}

/// Build parameter summary from inputSchema.properties.
///
/// Format: "param1*: type (desc), param2: type"
/// Required parameters are marked with an asterisk (*).
fn build_param_summary(input_schema: Option<&Value>) -> String {
    let Some(schema) = input_schema else {
        return "none".to_string();
    };

    let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
        return "none".to_string();
    };

    if properties.is_empty() {
        return "none".to_string();
    }

    // Get required params set for marking
    let required: HashSet<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    // Build "name: type (description)" entries
    let params: Vec<String> = properties
        .iter()
        .map(|(name, prop)| format_param(name, prop, required.contains(name.as_str())))
        .collect();

    params.join(", ")
}

/// Format a single parameter for the summary.
fn format_param(name: &str, prop: &Value, is_required: bool) -> String {
    let param_type = prop.get("type").and_then(|t| t.as_str()).unwrap_or("any");

    let brief_desc = prop
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| {
            // Take first sentence or first MAX_PARAM_DESC_LENGTH chars
            let end = d
                .find('.')
                .unwrap_or(MAX_PARAM_DESC_LENGTH)
                .min(MAX_PARAM_DESC_LENGTH);
            &d[..end.min(d.len())]
        })
        .unwrap_or("");

    let req_marker = if is_required { "*" } else { "" };

    if brief_desc.is_empty() {
        format!("{}{}: {}", name, req_marker, param_type)
    } else {
        format!("{}{}: {} ({})", name, req_marker, param_type, brief_desc)
    }
}

/// Construct the inference view string for reranker embedding.
///
/// Format: "TOOL: <name> | CONTEXT: <server_name> | FUNC: <description> | INPUTS: <param_summary>"
fn build_inference_view(name: &str, server: &str, desc: &str, params: &str) -> String {
    format!(
        "TOOL: {} | CONTEXT: {} | FUNC: {} | INPUTS: {}",
        name, server, desc, params
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_atomize_valid_mcp_response() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [{
                    "name": "calculate_sum",
                    "description": "Add two numbers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "a": { "type": "number", "description": "First number" },
                            "b": { "type": "number" }
                        },
                        "required": ["a", "b"]
                    }
                }]
            }
        });

        let tools = atomize_tools(&response, "math_server").unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "calculate_sum");
        assert_eq!(tools[0].server_origin, "math_server");
        assert!(tools[0].inference_view.contains("TOOL: calculate_sum"));
        assert!(tools[0].inference_view.contains("CONTEXT: math_server"));
        assert!(tools[0].inference_view.contains("a*: number"));
        assert!(tools[0].inference_view.contains("b*: number"));
    }

    #[test]
    fn test_atomize_missing_result_returns_error() {
        let response = json!({ "jsonrpc": "2.0" });
        let result = atomize_tools(&response, "server");

        assert!(result.is_err());
    }

    #[test]
    fn test_atomize_null_description_uses_empty() {
        let response = json!({
            "result": {
                "tools": [{
                    "name": "no_desc_tool",
                    "description": null,
                    "inputSchema": {}
                }]
            }
        });

        let tools = atomize_tools(&response, "server").unwrap();
        assert!(tools[0].inference_view.contains("FUNC:  |"));
    }

    #[test]
    fn test_atomize_missing_name_skips_tool() {
        let response = json!({
            "result": {
                "tools": [
                    { "description": "No name here" },
                    { "name": "valid_tool", "description": "Has name" }
                ]
            }
        });

        let tools = atomize_tools(&response, "server").unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "valid_tool");
    }

    #[test]
    fn test_truncate_long_description() {
        let long_desc = "A".repeat(600);
        let truncated = truncate_description(&long_desc);

        assert!(truncated.len() <= MAX_DESCRIPTION_LENGTH + 3);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_truncate_short_description() {
        let short_desc = "Short description";
        let result = truncate_description(short_desc);

        assert_eq!(result, short_desc);
    }

    #[test]
    fn test_build_param_summary_no_schema() {
        let summary = build_param_summary(None);
        assert_eq!(summary, "none");
    }

    #[test]
    fn test_build_param_summary_empty_properties() {
        let schema = json!({ "properties": {} });
        let summary = build_param_summary(Some(&schema));
        assert_eq!(summary, "none");
    }

    #[test]
    fn test_build_param_summary_marks_required() {
        let schema = json!({
            "properties": {
                "required_param": { "type": "string" },
                "optional_param": { "type": "number" }
            },
            "required": ["required_param"]
        });

        let summary = build_param_summary(Some(&schema));
        assert!(summary.contains("required_param*: string"));
        assert!(summary.contains("optional_param: number"));
        assert!(!summary.contains("optional_param*"));
    }

    #[test]
    fn test_inference_view_format() {
        let view = build_inference_view("my_tool", "my_server", "Does things", "x: int, y: str");
        assert_eq!(
            view,
            "TOOL: my_tool | CONTEXT: my_server | FUNC: Does things | INPUTS: x: int, y: str"
        );
    }

    #[test]
    fn test_empty_tools_array_returns_empty_vec() {
        let response = json!({ "result": { "tools": [] } });
        let tools = atomize_tools(&response, "server").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_nested_input_schema_types() {
        let response = json!({
            "result": {
                "tools": [{
                    "name": "complex_tool",
                    "inputSchema": {
                        "properties": {
                            "config": { "type": "object" },
                            "items": { "type": "array" }
                        }
                    }
                }]
            }
        });

        let tools = atomize_tools(&response, "server").unwrap();
        assert!(tools[0].inference_view.contains("config: object"));
        assert!(tools[0].inference_view.contains("items: array"));
    }

    #[test]
    fn test_param_description_truncation() {
        let schema = json!({
            "properties": {
                "verbose_param": {
                    "type": "string",
                    "description": "This is a very long description that should be truncated at the first period. It continues with more text."
                }
            }
        });

        let summary = build_param_summary(Some(&schema));
        assert!(summary.contains("verbose_param: string"));
        // Should truncate at first period (which is within 50 chars) or at 50 chars
        // The first period is at position 75, so it truncates at 50 chars
        assert!(summary.contains("This is a very long description that should be tr"));
        assert!(!summary.contains("It continues"));
    }
}
