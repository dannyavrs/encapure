//! Bi-encoder model for fast semantic similarity search.
//!
//! Uses BAAI/bge-base-en-v1.5 to produce 768-dimensional embeddings.
//! Unlike the cross-encoder (reranker), the bi-encoder encodes query and documents
//! independently, enabling pre-computation of document embeddings.
//!
//! # Session Pool Architecture
//! Like the reranker, the bi-encoder uses a session pool with lock-free queue
//! to enable concurrent query encoding without Mutex serialization.

use crate::error::{AppError, Result};
use crossbeam::queue::ArrayQueue;
use ndarray::{Array1, Array2};
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::Tensor,
};
use std::cell::UnsafeCell;
use std::path::Path;
use std::sync::Arc;
use tokenizers::Tokenizer;

/// Bi-encoder model pool for generating text embeddings with concurrent access.
///
/// # Design
/// The bi-encoder produces fixed-size embeddings (768-dim for BGE-base) that can be
/// compared via cosine similarity. This enables:
/// 1. Pre-computing document embeddings at startup
/// 2. Fast similarity search via vector operations (no model inference for docs)
///
/// # Session Pool
/// Uses the same lock-free pool pattern as RerankerModel to avoid Mutex serialization.
/// Each concurrent request acquires its own session for exclusive use.
pub struct BiEncoderModel {
    /// Pool of ONNX sessions - exclusive access guaranteed by ArrayQueue
    sessions: Vec<UnsafeCell<Session>>,
    /// Lock-free queue of available session indices
    available: Arc<ArrayQueue<usize>>,
    /// Shared tokenizer (thread-safe for encode operations)
    tokenizer: Tokenizer,
    max_length: usize,
    embedding_dim: usize,
}

impl BiEncoderModel {
    /// Load a pool of bi-encoder sessions.
    ///
    /// # Arguments
    /// * `model_path` - Path to the ONNX model file
    /// * `tokenizer_path` - Path to the tokenizer JSON file
    /// * `max_length` - Maximum sequence length (512 for BGE-base)
    /// * `pool_size` - Number of sessions to create
    /// * `intra_threads` - Threads per session for intra-op parallelism
    pub fn load_pool(
        model_path: &Path,
        tokenizer_path: &Path,
        max_length: usize,
        pool_size: usize,
        intra_threads: usize,
    ) -> Result<Self> {
        // Load tokenizer (shared across all sessions)
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| AppError::ModelError(format!("Failed to load bi-encoder tokenizer: {}", e)))?;

        // Read model file once
        let model_bytes = std::fs::read(model_path)
            .map_err(|e| AppError::ModelError(format!("Failed to read bi-encoder model: {}", e)))?;

        // Create pool of sessions
        let mut sessions = Vec::with_capacity(pool_size);
        let available = Arc::new(ArrayQueue::new(pool_size));

        for i in 0..pool_size {
            let session = Session::builder()
                .map_err(|e| AppError::ModelError(e.to_string()))?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| AppError::ModelError(e.to_string()))?
                .with_intra_threads(intra_threads)
                .map_err(|e| AppError::ModelError(e.to_string()))?
                .with_inter_threads(1)
                .map_err(|e| AppError::ModelError(e.to_string()))?
                .commit_from_memory(&model_bytes)
                .map_err(|e: ort::Error| AppError::ModelError(e.to_string()))?;

            sessions.push(UnsafeCell::new(session));
            available
                .push(i)
                .map_err(|_| AppError::ModelError("Failed to initialize bi-encoder session pool".into()))?;
        }

        tracing::info!(
            model = %model_path.display(),
            tokenizer = %tokenizer_path.display(),
            pool_size,
            intra_threads,
            max_length,
            "Bi-encoder session pool loaded"
        );

        Ok(Self {
            sessions,
            available,
            tokenizer,
            max_length,
            embedding_dim: 768, // BGE-base embedding dimension
        })
    }

    /// Legacy single-session load (for batch encoding at startup).
    ///
    /// Creates a pool of size 1 with specified intra_threads.
    pub fn load(model_path: &Path, tokenizer_path: &Path, max_length: usize) -> Result<Self> {
        // For startup batch encoding, use a single session with more threads
        Self::load_pool(model_path, tokenizer_path, max_length, 1, 4)
    }

    /// Acquire a session from the pool for exclusive use.
    ///
    /// Returns the session index, which MUST be released via `release_session()`.
    pub fn acquire_session(&self) -> Result<usize> {
        self.available
            .pop()
            .ok_or_else(|| AppError::ResourceError("No available bi-encoder sessions".into()))
    }

    /// Release a session back to the pool.
    pub fn release_session(&self, index: usize) {
        let _ = self.available.push(index);
    }

    /// Encode a single text into an embedding vector using a specific session.
    ///
    /// Uses mean pooling over token embeddings (excluding padding).
    pub fn encode_with_session(&self, session_idx: usize, text: &str) -> Result<Array1<f32>> {
        let texts = vec![text.to_string()];
        let embeddings = self.encode_batch_with_session(session_idx, &texts)?;
        Ok(embeddings.row(0).to_owned())
    }

    /// Encode a single text (convenience method that acquires/releases session automatically).
    pub fn encode(&self, text: &str) -> Result<Array1<f32>> {
        let session_idx = self.acquire_session()?;
        let result = self.encode_with_session(session_idx, text);
        self.release_session(session_idx);
        result
    }

    /// Encode a batch of texts using a specific session.
    ///
    /// # Returns
    /// Array2<f32> of shape (batch_size, embedding_dim)
    pub fn encode_batch_with_session(&self, session_idx: usize, texts: &[String]) -> Result<Array2<f32>> {
        if texts.is_empty() {
            return Ok(Array2::zeros((0, self.embedding_dim)));
        }

        // Tokenize all texts
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| AppError::ModelError(format!("Tokenization failed: {}", e)))?;

        let batch_size = encodings.len();

        // Find max length in this batch (capped at max_length)
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(self.max_length))
            .max()
            .unwrap_or(1);

        // Build padded input tensors
        let mut input_ids = vec![0i64; batch_size * max_len];
        let mut attention_mask = vec![0i64; batch_size * max_len];
        let token_type_ids = vec![0i64; batch_size * max_len];

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(max_len);

            for j in 0..len {
                input_ids[i * max_len + j] = ids[j] as i64;
                attention_mask[i * max_len + j] = mask[j] as i64;
            }
        }

        // Create tensors
        let shape = [batch_size, max_len];
        let input_ids_tensor = Tensor::from_array((shape, input_ids))
            .map_err(|e| AppError::ModelError(e.to_string()))?;
        let attention_mask_tensor = Tensor::from_array((shape, attention_mask.clone()))
            .map_err(|e| AppError::ModelError(e.to_string()))?;
        let token_type_ids_tensor = Tensor::from_array((shape, token_type_ids))
            .map_err(|e| AppError::ModelError(e.to_string()))?;

        // SAFETY: ArrayQueue guarantees exclusive access to this index.
        let session = unsafe { &mut *self.sessions[session_idx].get() };

        // Run inference
        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => token_type_ids_tensor,
            ])
            .map_err(|e| AppError::ModelError(format!("Bi-encoder inference failed: {}", e)))?;

        // Extract last_hidden_state (batch, seq_len, hidden_size)
        let hidden_state = outputs
            .get("last_hidden_state")
            .ok_or_else(|| AppError::ModelError("No 'last_hidden_state' output found".to_string()))?;

        let tensor = hidden_state
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::ModelError(e.to_string()))?;

        let (shape_info, data) = tensor;
        let hidden_size = shape_info[2] as usize;

        // Mean pooling with attention mask
        let mut embeddings = Array2::zeros((batch_size, hidden_size));

        for i in 0..batch_size {
            let mut sum = vec![0.0f32; hidden_size];
            let mut count = 0.0f32;

            for j in 0..max_len {
                if attention_mask[i * max_len + j] == 1 {
                    let base_idx = i * max_len * hidden_size + j * hidden_size;
                    for (k, sum_val) in sum.iter_mut().enumerate() {
                        *sum_val += data[base_idx + k];
                    }
                    count += 1.0;
                }
            }

            if count > 0.0 {
                for (k, sum_val) in sum.iter().enumerate() {
                    embeddings[[i, k]] = sum_val / count;
                }
            }

            // L2 normalize the embedding
            let norm: f32 = embeddings.row(i).iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                let mut row = embeddings.row_mut(i);
                for val in row.iter_mut() {
                    *val /= norm;
                }
            }
        }

        Ok(embeddings)
    }

    /// Encode a batch of texts (convenience method that acquires/releases session automatically).
    pub fn encode_batch(&self, texts: &[String]) -> Result<Array2<f32>> {
        let session_idx = self.acquire_session()?;
        let result = self.encode_batch_with_session(session_idx, texts);
        self.release_session(session_idx);
        result
    }

    /// Compute cosine similarity between a query embedding and multiple document embeddings.
    ///
    /// # Arguments
    /// * `query_embedding` - Normalized query embedding (1D array)
    /// * `doc_embeddings` - Normalized document embeddings (2D array: num_docs × embedding_dim)
    ///
    /// # Returns
    /// Vector of similarity scores (one per document)
    pub fn cosine_similarity(
        query_embedding: &Array1<f32>,
        doc_embeddings: &Array2<f32>,
    ) -> Vec<f32> {
        // Since embeddings are L2-normalized, cosine similarity = dot product
        doc_embeddings
            .outer_iter()
            .map(|doc| query_embedding.dot(&doc))
            .collect()
    }
}

// SAFETY: BiEncoderModel is Send + Sync because:
// - ArrayQueue is lock-free and thread-safe (crossbeam guarantee)
// - ArrayQueue::pop() returns each index to at most one caller at a time
// - ArrayQueue::push() returns the index to the pool for reuse
// - Between pop and push, only one thread can access each UnsafeCell<Session>
// - Tokenizer's encode methods are thread-safe
unsafe impl Send for BiEncoderModel {}
unsafe impl Sync for BiEncoderModel {}
