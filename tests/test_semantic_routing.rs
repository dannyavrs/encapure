//! Integration tests for Semantic Routing feature.
//!
//! Tests verify that the search endpoint correctly ranks tools by semantic relevance.
//! Run with: cargo test --test test_semantic_routing -- --ignored --test-threads=1

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use encapure::{search_handler, AppState, Config};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create test router with search endpoint.
fn create_test_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/search", post(search_handler))
        .with_state(state)
}

/// Helper to make JSON POST request.
async fn json_post(app: Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));

    (status, body)
}

// ============================================================================
// Validation Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_empty_query_returns_400() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, response) = json_post(app, "/search", json!({ "query": "" })).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"]
        .as_str()
        .unwrap_or("")
        .to_lowercase()
        .contains("empty"));
}

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_zero_top_k_returns_400() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, response) =
        json_post(app, "/search", json!({ "query": "test", "top_k": 0 })).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"]
        .as_str()
        .unwrap_or("")
        .to_lowercase()
        .contains("top_k"));
}

// ============================================================================
// Semantic Relevance Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_server_status_returns_devops_tools() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, response) = json_post(
        app,
        "/search",
        json!({
            "query": "I need to verify if the server is running",
            "top_k": 3
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);

    // Collect top result names
    let top_names: Vec<&str> = results
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();

    // DevOps tools should dominate the results
    let devops_tools = [
        "aws_list_instances",
        "k8s_get_pods",
        "check_service_health",
        "get_server_metrics",
    ];
    let devops_count = top_names
        .iter()
        .filter(|name| devops_tools.contains(name))
        .count();

    assert!(
        devops_count >= 2,
        "Expected at least 2 DevOps tools in top 3, got: {:?}",
        top_names
    );

    // Irrelevant tools should NOT appear in top results
    let irrelevant_tools = ["read_file", "write_file", "send_slack_message", "email_send"];
    for name in &top_names {
        assert!(
            !irrelevant_tools.contains(name),
            "Irrelevant tool '{}' should not be in top results for server status query",
            name
        );
    }
}

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_file_operations_returns_filesystem_tools() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, response) = json_post(
        app,
        "/search",
        json!({
            "query": "Read the contents of a configuration file",
            "top_k": 3
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();
    let top_name = results[0]["name"].as_str().unwrap();

    // read_file should be the top result or at least in top 3
    let file_tools = ["read_file", "file_metadata", "search_files"];
    let top_names: Vec<&str> = results
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();

    let file_count = top_names
        .iter()
        .filter(|name| file_tools.contains(name))
        .count();

    assert!(
        file_count >= 1,
        "Expected at least 1 file tool in top 3 for file reading query, got: {:?}",
        top_names
    );

    // read_file should ideally be top result
    if top_name != "read_file" {
        println!(
            "Note: read_file was not top result, got '{}' instead. This may be acceptable.",
            top_name
        );
    }
}

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_returns_raw_definition() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, response) = json_post(
        app,
        "/search",
        json!({
            "query": "Create a new Jira ticket for a bug",
            "top_k": 1
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);

    // Verify raw_definition contains the MCP schema
    let raw_def = &results[0]["raw_definition"];
    assert!(
        raw_def["name"].is_string(),
        "raw_definition should contain name"
    );
    assert!(
        raw_def["description"].is_string() || raw_def["description"].is_null(),
        "raw_definition should contain description"
    );
    assert!(
        raw_def["inputSchema"].is_object(),
        "raw_definition should contain inputSchema"
    );
}

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_scores_are_descending() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, response) = json_post(
        app,
        "/search",
        json!({
            "query": "Send a message to the team",
            "top_k": 5
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();
    let scores: Vec<f64> = results
        .iter()
        .map(|r| r["score"].as_f64().unwrap())
        .collect();

    for i in 1..scores.len() {
        assert!(
            scores[i - 1] >= scores[i],
            "Scores should be in descending order: {:?}",
            scores
        );
    }
}

#[tokio::test]
#[ignore = "Requires model files and TOOLS_PATH - run with --ignored"]
async fn test_search_default_top_k() {
    std::env::set_var("TOOLS_PATH", "tests/data/comprehensive_mock_tools.json");
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    // Don't specify top_k, should default to 3
    let (status, response) = json_post(
        app,
        "/search",
        json!({
            "query": "Search for something"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 3, "Default top_k should be 3");
}
