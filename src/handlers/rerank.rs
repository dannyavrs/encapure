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

    let total_docs = request.documents.len();

    // Acquire semaphore with timeout (503 if service overloaded)
    // Extended timeout for large batch requests
    let _permit = tokio::time::timeout(Duration::from_secs(30), state.semaphore.acquire())
        .await
        .map_err(|_| {
            AppError::ResourceError("Service temporarily overloaded, please retry".to_string())
        })?
        .map_err(|_| AppError::ResourceError("Semaphore closed".to_string()))?;

    // Acquire session BEFORE spawn_blocking to prevent race conditions.
    // Since semaphore has N permits and pool has N sessions, this always succeeds
    // immediately when we hold a permit.
    let session_idx = state.model.acquire_session()?;

    // Clone Arcs for the blocking task
    let model = Arc::clone(&state.model);
    let tokenizer = Arc::clone(&state.tokenizer);
    let documents = request.documents.clone();
    let query = request.query.clone();
    let chunk_size = state.config.batch_size;

    // Run CPU-bound work in blocking task pool with timeout
    // Extended timeout for large requests (5 minutes)
    let inference_timeout = Duration::from_secs(300);
    let result = tokio::time::timeout(
        inference_timeout,
        tokio::task::spawn_blocking(move || {
            let mut all_scores: Vec<f32> = Vec::with_capacity(documents.len());

            // Process documents in batches for memory efficiency
            for chunk in documents.chunks(chunk_size) {
                // Tokenize this batch
                let (input_ids, attention_mask, token_type_ids) =
                    tokenizer.tokenize_pairs(&query, chunk)?;

                // Inference with pre-acquired session
                let batch_scores = model.inference_with_session(
                    session_idx,
                    input_ids,
                    attention_mask,
                    token_type_ids,
                )?;

                all_scores.extend(batch_scores);
            }

            Ok::<Vec<f32>, AppError>(all_scores)
        }),
    )
    .await;

    // Always release the session, even on timeout or error
    state.model.release_session(session_idx);

    // Now handle the result
    let scores = result
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

    tracing::debug!(total_docs, "Rerank completed");

    metrics::counter!("rerank_requests_total").increment(1);
    metrics::histogram!("rerank_batch_size").record(total_docs as f64);

    Ok(Json(RerankResponse { results }))
}

/// Sigmoid activation: 1 / (1 + e^-x)
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
