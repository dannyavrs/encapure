//! Integration tests for Encapure reranking service.
//!
//! These tests verify the API behavior and error handling.
//! Run with: cargo test -- --test-threads=1

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use encapure::{
    handlers::{health_handler, ready_handler, rerank_handler},
    AppState, Config,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create a test router with the rerank endpoint.
fn create_test_app(state: Arc<AppState>) -> Router {
    use axum::routing::{get, post};

    Router::new()
        .route("/rerank", post(rerank_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .with_state(state)
}

/// Helper to make a JSON request to the router.
async fn json_request(
    app: Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let req = match method {
        "GET" => Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
        "POST" => Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.unwrap_or(json!({})).to_string()))
            .unwrap(),
        _ => panic!("Unsupported method"),
    };

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));

    (status, body)
}

// ============================================================================
// Health Endpoint Tests
// ============================================================================

#[tokio::test]
async fn test_health_endpoint_returns_200() {
    use axum::routing::get;

    let app = Router::new().route("/health", get(health_handler));
    let (status, body) = json_request(app, "GET", "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
}

// ============================================================================
// Validation Tests (don't require model)
// ============================================================================

// Note: The following tests require a loaded model to run.
// They are marked with #[ignore] and can be run with:
// cargo test -- --ignored --test-threads=1
//
// To run these tests, first ensure you have:
// 1. Run python/export_model.py to create the model
// 2. Set MODEL_PATH and TOKENIZER_PATH environment variables

#[tokio::test]
#[ignore = "Requires model files - run with --ignored after exporting model"]
async fn test_rerank_empty_query_returns_400() {
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let body = json!({
        "query": "",
        "documents": ["doc1", "doc2"]
    });

    let (status, response) = json_request(app, "POST", "/rerank", Some(body)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("empty"));
}

#[tokio::test]
#[ignore = "Requires model files - run with --ignored after exporting model"]
async fn test_rerank_empty_documents_returns_400() {
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let body = json!({
        "query": "test query",
        "documents": []
    });

    let (status, response) = json_request(app, "POST", "/rerank", Some(body)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("empty"));
}

#[tokio::test]
#[ignore = "Requires model files - run with --ignored after exporting model"]
async fn test_rerank_too_many_documents_returns_400() {
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    // Create 101 documents (exceeds limit of 100)
    let documents: Vec<String> = (0..101).map(|i| format!("document {}", i)).collect();

    let body = json!({
        "query": "test query",
        "documents": documents
    });

    let (status, response) = json_request(app, "POST", "/rerank", Some(body)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("100"));
}

#[tokio::test]
#[ignore = "Requires model files - run with --ignored after exporting model"]
async fn test_rerank_success_returns_sorted_results() {
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let body = json!({
        "query": "What is machine learning?",
        "documents": [
            "Machine learning is a subset of artificial intelligence",
            "The weather is nice today",
            "Deep learning uses neural networks"
        ]
    });

    let (status, response) = json_request(app, "POST", "/rerank", Some(body)).await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);

    // Verify results are sorted by score descending
    let scores: Vec<f64> = results
        .iter()
        .map(|r| r["score"].as_f64().unwrap())
        .collect();

    for i in 1..scores.len() {
        assert!(
            scores[i - 1] >= scores[i],
            "Results should be sorted by score descending"
        );
    }

    // Verify each result has required fields
    for result in results {
        assert!(result["index"].is_number());
        assert!(result["score"].is_number());
        assert!(result["document"].is_string());
    }
}

#[tokio::test]
#[ignore = "Requires model files - run with --ignored after exporting model"]
async fn test_rerank_semantic_relevance() {
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let body = json!({
        "query": "What is machine learning?",
        "documents": [
            "The recipe calls for flour and sugar",
            "Machine learning is a type of artificial intelligence that learns from data",
            "My favorite color is blue"
        ]
    });

    let (status, response) = json_request(app, "POST", "/rerank", Some(body)).await;

    assert_eq!(status, StatusCode::OK);

    let results = response["results"].as_array().unwrap();

    // The ML-related document should rank highest
    let top_result = &results[0];
    assert!(
        top_result["document"]
            .as_str()
            .unwrap()
            .contains("Machine learning"),
        "ML document should rank highest for ML query"
    );
}

#[tokio::test]
#[ignore = "Requires model files - run with --ignored after exporting model"]
async fn test_ready_endpoint_returns_200_after_warmup() {
    let config = Config::from_env().expect("Failed to load config");
    let state = Arc::new(AppState::new(config).expect("Failed to create AppState"));
    let app = create_test_app(state);

    let (status, body) = json_request(app, "GET", "/ready", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

// ============================================================================
// Unit Tests for Sigmoid Function
// ============================================================================

#[test]
fn test_sigmoid_zero() {
    let result = sigmoid(0.0);
    assert!((result - 0.5).abs() < 0.0001);
}

#[test]
fn test_sigmoid_positive() {
    let result = sigmoid(10.0);
    assert!(result > 0.99);
}

#[test]
fn test_sigmoid_negative() {
    let result = sigmoid(-10.0);
    assert!(result < 0.01);
}

/// Sigmoid activation: 1 / (1 + e^-x)
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
