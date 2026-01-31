//! Bi-encoder model for fast semantic similarity search.
//!
//! Uses BAAI/bge-base-en-v1.5 to produce 768-dimensional embeddings.
//! Unlike the cross-encoder (reranker), the bi-encoder encodes query and documents
//! independently, enabling pre-computation of document embeddings.

use crate::error::{AppError, Result};
use ndarray::{Array1, Array2};
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::Tensor,
};
use std::path::Path;
use tokenizers::Tokenizer;

/// Bi-encoder model for generating text embeddings.
///
/// # Design
/// The bi-encoder produces fixed-size embeddings (768-dim for BGE-base) that can be
/// compared via cosine similarity. This enables:
/// 1. Pre-computing document embeddings at startup
/// 2. Fast similarity search via vector operations (no model inference for docs)
pub struct BiEncoderModel {
    session: Session,
    tokenizer: Tokenizer,
    max_length: usize,
    embedding_dim: usize,
}

impl BiEncoderModel {
    /// Load bi-encoder model and tokenizer.
    ///
    /// # Arguments
    /// * `model_path` - Path to the ONNX model file
    /// * `tokenizer_path` - Path to the tokenizer JSON file
    /// * `max_length` - Maximum sequence length (512 for BGE-base)
    pub fn load(model_path: &Path, tokenizer_path: &Path, max_length: usize) -> Result<Self> {
        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| AppError::ModelError(format!("Failed to load bi-encoder tokenizer: {}", e)))?;

        // Load ONNX session with optimizations
        let session = Session::builder()
            .map_err(|e| AppError::ModelError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| AppError::ModelError(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| AppError::ModelError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e| AppError::ModelError(format!("Failed to load bi-encoder model: {}", e)))?;

        tracing::info!(
            model = %model_path.display(),
            tokenizer = %tokenizer_path.display(),
            max_length,
            "Bi-encoder model loaded"
        );

        Ok(Self {
            session,
            tokenizer,
            max_length,
            embedding_dim: 768, // BGE-base embedding dimension
        })
    }

    /// Encode a single text into an embedding vector.
    ///
    /// Uses mean pooling over token embeddings (excluding padding).
    pub fn encode(&mut self, text: &str) -> Result<Array1<f32>> {
        let texts = vec![text.to_string()];
        let embeddings = self.encode_batch(&texts)?;
        Ok(embeddings.row(0).to_owned())
    }

    /// Encode a batch of texts into embedding vectors.
    ///
    /// # Returns
    /// Array2<f32> of shape (batch_size, embedding_dim)
    pub fn encode_batch(&mut self, texts: &[String]) -> Result<Array2<f32>> {
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

        // Run inference
        let outputs = self
            .session
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
