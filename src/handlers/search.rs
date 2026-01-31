//! Semantic search handler for tool discovery.
//!
//! This handler enables AI agents to discover relevant tools by semantic matching
//! against a query. Uses a two-stage retrieval architecture for performance:
//!
//! **Stage 1 (Bi-encoder)**: Fast cosine similarity search using pre-computed embeddings
//! **Stage 2 (Cross-encoder)**: Accurate reranking on top candidates only
//!
//! This reduces latency from O(n × inference) to O(1 + k × inference) where k << n.

use crate::error::{AppError, Result};
use crate::inference::BiEncoderModel;
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Default number of results to return
fn default_top_k() -> usize {
    3
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// The natural language query to match against tools
    pub query: String,
    /// Number of top results to return (default: 3)
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    /// The tool name
    pub name: String,
    /// Relevance score (0.0 to 1.0, higher is more relevant)
    pub score: f32,
    /// The original MCP tool definition (for agent execution)
    pub raw_definition: Value,
}

/// POST /search - Find tools relevant to a natural language query.
///
/// Uses a two-stage retrieval architecture for fast, accurate tool discovery:
///
/// # Two-Stage Retrieval Flow
/// 1. **Validation**: Check query non-empty, top_k > 0
/// 2. **Stage 1 (Bi-encoder)**: Compute query embedding, cosine similarity with
///    pre-computed tool embeddings, retrieve top-N candidates (fast: ~10ms)
/// 3. **Stage 2 (Cross-encoder)**: Run reranker only on N candidates (accurate)
/// 4. Apply sigmoid, sort descending, return top-K results
///
/// This reduces latency from O(n × inference) to O(1 + k × inference).
pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>> {
    let start_time = std::time::Instant::now();

    // Validation
    if request.query.is_empty() {
        return Err(AppError::ValidationError(
            "Query cannot be empty".to_string(),
        ));
    }

    if request.top_k == 0 {
        return Err(AppError::ValidationError(
            "top_k must be at least 1".to_string(),
        ));
    }

    // Check if tools are loaded
    if state.tools.is_empty() {
        return Err(AppError::ValidationError(
            "No tools loaded. Set TOOLS_PATH environment variable.".to_string(),
        ));
    }

    let tools = Arc::clone(&state.tools);
    let top_k = request.top_k.min(tools.len());
    let retrieval_candidates = state.config.retrieval_candidates.min(tools.len());

    // =========================================================================
    // STAGE 1: Bi-encoder fast retrieval (cosine similarity)
    // =========================================================================

    // Acquire bi-encoder session BEFORE spawn_blocking (lock-free pool access)
    let bi_encoder_session_idx = state.bi_encoder.acquire_session()?;

    let bi_encoder = Arc::clone(&state.bi_encoder);
    let tool_embeddings = Arc::clone(&state.tool_embeddings);
    let query_for_biencoder = request.query.clone();

    // Compute query embedding (fast - single forward pass)
    let stage1_result = tokio::task::spawn_blocking(move || {
        let query_embedding = bi_encoder.encode_with_session(bi_encoder_session_idx, &query_for_biencoder)?;

        // Compute cosine similarities with all pre-computed tool embeddings
        let similarities = BiEncoderModel::cosine_similarity(&query_embedding, &tool_embeddings);

        // Get top-N candidate indices
        let mut indexed_sims: Vec<(usize, f32)> =
            similarities.into_iter().enumerate().collect();
        indexed_sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let candidates: Vec<usize> = indexed_sims
            .into_iter()
            .take(retrieval_candidates)
            .map(|(idx, _)| idx)
            .collect();

        Ok::<Vec<usize>, AppError>(candidates)
    })
    .await;

    // Release bi-encoder session after blocking task completes
    state.bi_encoder.release_session(bi_encoder_session_idx);

    // Handle the result
    let stage1_result = stage1_result
        .map_err(|e| AppError::ModelError(format!("Stage 1 task join error: {}", e)))??;

    let stage1_time = start_time.elapsed();
    tracing::debug!(
        stage1_ms = stage1_time.as_millis(),
        candidates = stage1_result.len(),
        "Stage 1 (bi-encoder) completed"
    );

    // =========================================================================
    // STAGE 2: Cross-encoder reranking on candidates only
    // =========================================================================

    // Acquire semaphore with timeout (503 if service overloaded)
    let _permit = tokio::time::timeout(Duration::from_secs(10), state.semaphore.acquire())
        .await
        .map_err(|_| {
            AppError::ResourceError("Service temporarily overloaded, please retry".to_string())
        })?
        .map_err(|_| AppError::ResourceError("Semaphore closed".to_string()))?;

    // Acquire session BEFORE spawn_blocking
    let session_idx = state.model.acquire_session()?;

    // Clone Arcs for the blocking task
    let model = Arc::clone(&state.model);
    let tokenizer = Arc::clone(&state.tokenizer);
    let query = request.query.clone();
    let batch_size = state.config.batch_size;
    let candidate_indices = stage1_result;
    let tools_for_rerank = Arc::clone(&tools);

    // Run cross-encoder only on candidate tools
    let result = tokio::task::spawn_blocking(move || {
        // Get inference views only for candidate tools
        let documents: Vec<String> = candidate_indices
            .iter()
            .map(|&idx| tools_for_rerank[idx].inference_view.clone())
            .collect();

        let mut all_scores: Vec<f32> = Vec::with_capacity(documents.len());

        // Process in batches for memory efficiency
        for chunk in documents.chunks(batch_size) {
            let chunk_vec: Vec<String> = chunk.to_vec();
            let (input_ids, attention_mask, _) = tokenizer.tokenize_pairs(&query, &chunk_vec)?;
            let batch_scores =
                model.inference_with_session(session_idx, input_ids, attention_mask)?;
            all_scores.extend(batch_scores);
        }

        // Map scores back to original tool indices
        let scored_candidates: Vec<(usize, f32)> = candidate_indices
            .into_iter()
            .zip(all_scores.into_iter())
            .collect();

        Ok::<Vec<(usize, f32)>, AppError>(scored_candidates)
    })
    .await;

    // Release session after blocking task completes
    state.model.release_session(session_idx);

    // Handle the result
    let scored_candidates =
        result.map_err(|e| AppError::ModelError(format!("Stage 2 task join error: {}", e)))??;

    let stage2_time = start_time.elapsed();
    tracing::debug!(
        stage2_ms = (stage2_time - stage1_time).as_millis(),
        "Stage 2 (cross-encoder) completed"
    );

    // Apply sigmoid and sort by score descending
    let mut final_scores: Vec<(usize, f32)> = scored_candidates
        .into_iter()
        .map(|(idx, logit)| (idx, sigmoid(logit)))
        .collect();

    final_scores.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Take top-K and build response
    let results: Vec<SearchResult> = final_scores
        .into_iter()
        .take(top_k)
        .map(|(idx, score)| {
            let tool = &state.tools[idx];
            SearchResult {
                name: tool.name.clone(),
                score,
                raw_definition: tool.raw_definition.clone(),
            }
        })
        .collect();

    let total_time = start_time.elapsed();
    tracing::info!(
        query = %request.query,
        top_k,
        retrieval_candidates,
        total_ms = total_time.as_millis(),
        stage1_ms = stage1_time.as_millis(),
        stage2_ms = (total_time - stage1_time).as_millis(),
        "Search completed (two-stage retrieval)"
    );

    metrics::counter!("search_requests_total").increment(1);
    metrics::histogram!("search_latency_ms").record(total_time.as_millis() as f64);

    Ok(Json(SearchResponse { results }))
}

/// Sigmoid activation: 1 / (1 + e^-x)
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
