use crate::error::{AppError, Result};
use crossbeam::queue::ArrayQueue;
use ndarray::Array2;
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::Tensor,
};
use std::cell::UnsafeCell;
use std::path::Path;
use std::sync::Arc;

/// A pool of ONNX Runtime Sessions for parallel inference.
///
/// # Design Rationale
/// ONNX Session::run requires `&mut self`, but we need concurrent access.
/// Instead of using a Mutex (which serializes all requests), we create N
/// independent Session instances - one per CPU core. Each concurrent request
/// acquires an exclusive session from the pool via a lock-free queue.
///
/// # Safety
/// The pool guarantees that each session index is held by at most one thread
/// at a time. The ArrayQueue provides this guarantee through atomic operations.
/// Sessions themselves are not shared - each inference gets exclusive access
/// to its acquired session via UnsafeCell, which is safe because the ArrayQueue
/// ensures only one thread holds each index at any time.
pub struct RerankerModel {
    /// Pool of ONNX sessions - exclusive access guaranteed by ArrayQueue
    sessions: Vec<UnsafeCell<Session>>,
    /// Lock-free queue of available session indices
    available: Arc<ArrayQueue<usize>>,
}

impl RerankerModel {
    /// Load a pool of ONNX sessions with Level3 optimization.
    ///
    /// # Arguments
    /// * `model_path` - Path to the ONNX model file
    /// * `pool_size` - Number of sessions to create (typically CPU core count)
    /// * `intra_threads` - Threads per session for intra-op parallelism
    ///
    /// # Thread Configuration
    /// - `intra_threads=1`: Best for high-concurrency (many small requests)
    /// - `intra_threads=4+`: Best for batch processing (fewer requests, larger batches)
    pub fn load_pool(model_path: &Path, pool_size: usize, intra_threads: usize) -> Result<Self> {
        // Read model file once
        let model_bytes = std::fs::read(model_path)
            .map_err(|e| AppError::ModelError(format!("Failed to read model file: {}", e)))?;

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
            // Mark this session index as available
            available
                .push(i)
                .map_err(|_| AppError::ModelError("Failed to initialize session pool".into()))?;
        }

        tracing::info!(
            path = %model_path.display(),
            pool_size,
            intra_threads,
            "ONNX session pool loaded successfully"
        );

        Ok(Self {
            sessions,
            available,
        })
    }

    /// Acquire a session from the pool for exclusive use.
    ///
    /// Returns the session index, which MUST be released via `release_session()`.
    /// This should be called BEFORE `spawn_blocking` to reserve a session
    /// while holding the semaphore permit, preventing race conditions.
    pub fn acquire_session(&self) -> Result<usize> {
        self.available
            .pop()
            .ok_or_else(|| AppError::ResourceError("No available sessions in pool".into()))
    }

    /// Release a session back to the pool.
    ///
    /// Must be called after inference completes (or on error/timeout).
    pub fn release_session(&self, index: usize) {
        // This should never fail since we only release indices we acquired
        let _ = self.available.push(index);
    }

    /// Run inference on tokenized inputs using a pre-acquired session.
    ///
    /// The caller MUST have acquired the session via `acquire_session()` and
    /// MUST release it via `release_session()` after this call completes.
    pub fn inference_with_session(
        &self,
        session_idx: usize,
        input_ids: Array2<i64>,
        attention_mask: Array2<i64>,
    ) -> Result<Vec<f32>> {
        let batch_size = input_ids.nrows();
        let seq_len = input_ids.ncols();

        self.run_inference_on_session(
            session_idx,
            input_ids,
            attention_mask,
            batch_size,
            seq_len,
        )
    }

    /// Run inference on tokenized inputs (convenience method for warmup/single requests).
    ///
    /// This method acquires a session, runs inference, and releases automatically.
    /// For high-concurrency scenarios, prefer `acquire_session` + `inference_with_session`
    /// + `release_session` to control session lifetime explicitly.
    pub fn inference(
        &self,
        input_ids: Array2<i64>,
        attention_mask: Array2<i64>,
    ) -> Result<Vec<f32>> {
        let session_idx = self.acquire_session()?;
        let result = self.inference_with_session(session_idx, input_ids, attention_mask);
        self.release_session(session_idx);
        result
    }

    /// Internal method that runs inference on a specific session.
    fn run_inference_on_session(
        &self,
        session_idx: usize,
        input_ids: Array2<i64>,
        attention_mask: Array2<i64>,
        batch_size: usize,
        seq_len: usize,
    ) -> Result<Vec<f32>> {
        // Get raw data as contiguous vectors
        let input_ids_vec: Vec<i64> = input_ids.iter().cloned().collect();
        let attention_mask_vec: Vec<i64> = attention_mask.iter().cloned().collect();

        // Create tensors with shape info
        let shape = [batch_size, seq_len];
        let input_ids_tensor = Tensor::from_array((shape, input_ids_vec))
            .map_err(|e| AppError::ModelError(e.to_string()))?;
        let attention_mask_tensor = Tensor::from_array((shape, attention_mask_vec))
            .map_err(|e| AppError::ModelError(e.to_string()))?;

        // SAFETY: ArrayQueue guarantees exclusive access to this index.
        // Only one thread can hold session_idx between acquire_session() and release_session().
        // The ArrayQueue acts as our synchronization primitive, making the UnsafeCell access safe.
        let session = unsafe { &mut *self.sessions[session_idx].get() };

        // BGE reranker requires input_ids and attention_mask
        let outputs = session
            .run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
            ])
            .map_err(|e| AppError::ModelError(e.to_string()))?;

        // Extract logits from output
        let logits_tensor = outputs
            .get("logits")
            .ok_or_else(|| AppError::ModelError("No 'logits' output found".to_string()))?;

        let logits = logits_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::ModelError(e.to_string()))?;

        // Extract relevance scores from the tensor data
        let (_shape, data) = logits;
        let scores: Vec<f32> = data.to_vec();

        // Take first `batch_size` scores if output is flattened
        Ok(scores.into_iter().take(batch_size).collect())
    }

}

// SAFETY: RerankerModel is Send + Sync because:
// - ArrayQueue is lock-free and thread-safe (crossbeam guarantee)
// - ArrayQueue::pop() returns each index to at most one caller at a time
// - ArrayQueue::push() returns the index to the pool for reuse
// - Between pop and push, only one thread can access each UnsafeCell<Session>
// - This provides the same mutual exclusion guarantee as a Mutex, but without blocking
// - Sessions are never accessed without first acquiring their index from the queue
unsafe impl Send for RerankerModel {}
unsafe impl Sync for RerankerModel {}
