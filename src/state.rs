use crate::config::Config;
use crate::error::Result;
use crate::inference::{RerankerModel, TokenizerWrapper};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Application state shared across all request handlers.
/// Uses Arc for zero-copy sharing - Session and Tokenizer are thread-safe.
pub struct AppState {
    pub model: Arc<RerankerModel>,
    pub tokenizer: Arc<TokenizerWrapper>,
    pub semaphore: Arc<Semaphore>,
    /// Flag indicating the service is ready (model loaded and warmed up)
    pub ready: AtomicBool,
    #[allow(dead_code)]
    pub config: Arc<Config>,
}

impl AppState {
    /// Initialize application state.
    ///
    /// # Semaphore Strategy
    /// Permits = physical CPU cores. Each inference request acquires one permit,
    /// ensuring we never have more concurrent inferences than physical cores.
    /// This prevents thread thrashing when ONNX intra_threads=1.
    ///
    /// # Why Physical Cores Only?
    /// Hyperthreads (logical cores) share the same physical core's execution units.
    /// Running CPU-intensive inference on both hyperthreads causes contention,
    /// cache thrashing, and context switching overhead.
    ///
    /// # Session Pool
    /// We create one ONNX session per physical core, enabling true parallel
    /// inference without contention.
    pub fn new(config: Config) -> Result<Self> {
        // Detect available CPU cores
        let logical_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        // Use physical cores only (logical / 2 on hyperthreaded systems)
        // Can be overridden via POOL_SIZE env var
        let num_cores = config.pool_size.unwrap_or_else(|| (logical_cores / 2).max(1));

        tracing::info!(
            logical_cores,
            physical_cores = num_cores,
            pool_size_override = config.pool_size,
            "Configured session pool size"
        );

        // Load model pool and tokenizer
        let model = RerankerModel::load_pool(&config.model_path, num_cores)?;
        let tokenizer =
            TokenizerWrapper::load(&config.tokenizer_path, config.max_sequence_length)?;

        let state = Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            semaphore: Arc::new(Semaphore::new(num_cores)),
            ready: AtomicBool::new(false),
            config: Arc::new(config),
        };

        // Warmup the model with a dummy inference
        state.warmup()?;

        // Mark as ready after successful warmup
        state.ready.store(true, Ordering::SeqCst);

        Ok(state)
    }

    /// Run a warmup inference to trigger lazy initialization in ONNX Runtime.
    /// This ensures the first real request doesn't suffer cold-start latency.
    fn warmup(&self) -> Result<()> {
        tracing::info!("Running model warmup...");

        let warmup_query = "warmup query";
        let warmup_docs = vec!["warmup document".to_string()];

        // Tokenize
        let (input_ids, attention_mask, token_type_ids) =
            self.tokenizer.tokenize_pairs(warmup_query, &warmup_docs)?;

        // Run inference (discard results)
        let _ = self.model.inference(input_ids, attention_mask, token_type_ids)?;

        tracing::info!("Model warmup completed successfully");
        Ok(())
    }

    /// Check if the service is ready to handle requests.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}
