use crate::error::{AppError, Result};
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct RerankRequest {
    pub query: String,
    pub documents: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RerankResponse {
    pub results: Vec<RankedDocument>,
}

#[derive(Debug, Serialize)]
pub struct RankedDocument {
    pub index: usize,
    pub score: f32,
    pub document: String,
}

/// POST /rerank - Rerank documents by relevance to query.
///
/// # Flow
/// 1. Validate input
/// 2. Acquire semaphore permit (blocks if all CPUs busy)
/// 3. Tokenize query-document pairs
/// 4. Run ONNX inference
/// 5. Apply sigmoid and sort by score
pub async fn rerank_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RerankRequest>,
) -> Result<Json<RerankResponse>> {
    // Validation
    if request.query.is_empty() {
        return Err(AppError::ValidationError(
            "Query cannot be empty".to_string(),
        ));
    }
    if request.documents.is_empty() {
        return Err(AppError::ValidationError(
            "Documents list cannot be empty".to_string(),
        ));
    }
    let max_docs = state.config.max_documents;
    if request.documents.len() > max_docs {
        return Err(AppError::ValidationError(
            format!("Maximum {} documents per request", max_docs),
        ));
    }

    let batch_size = request.documents.len();

    // Acquire semaphore with timeout (503 if service overloaded)
    let _permit = tokio::time::timeout(Duration::from_secs(5), state.semaphore.acquire())
        .await
        .map_err(|_| {
            AppError::ResourceError("Service temporarily overloaded, please retry".to_string())
        })?
        .map_err(|_| AppError::ResourceError("Semaphore closed".to_string()))?;

    // Clone Arcs for the blocking task
    let model = Arc::clone(&state.model);
    let tokenizer = Arc::clone(&state.tokenizer);
    let documents = request.documents.clone();
    let query = request.query.clone();

    // Run CPU-bound work in blocking task pool with timeout
    let inference_timeout = Duration::from_secs(30);
    let scores = tokio::time::timeout(
        inference_timeout,
        tokio::task::spawn_blocking(move || {
            // Tokenize
            let (input_ids, attention_mask, token_type_ids) =
                tokenizer.tokenize_pairs(&query, &documents)?;

            // Inference
            model.inference(input_ids, attention_mask, token_type_ids)
        }),
    )
    .await
    .map_err(|_| AppError::ResourceError("Inference timeout exceeded (30s)".to_string()))?
    .map_err(|e| AppError::ModelError(format!("Task join error: {}", e)))??;

    // Apply sigmoid and create ranked results
    let mut results: Vec<RankedDocument> = scores
        .into_iter()
        .zip(request.documents.into_iter())
        .enumerate()
        .map(|(index, (logit, document))| RankedDocument {
            index,
            score: sigmoid(logit),
            document,
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    tracing::debug!(batch_size, "Rerank completed");

    metrics::counter!("rerank_requests_total").increment(1);
    metrics::histogram!("rerank_batch_size").record(batch_size as f64);

    Ok(Json(RerankResponse { results }))
}

/// Sigmoid activation: 1 / (1 + e^-x)
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
