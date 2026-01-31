use crate::config::Config;
use crate::error::{AppError, Result};
use crate::inference::{BiEncoderModel, RerankerModel, TokenizerWrapper};
use crate::ingestion::{atomize_tools, EncapureTool};
use crate::persistence::{save_embeddings_cache, try_load_embeddings_cache};
use ndarray::Array2;
use std::path::Path;
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
    pub config: Arc<Config>,
    /// Pre-loaded tools for semantic routing (empty if no TOOLS_PATH configured)
    pub tools: Arc<Vec<EncapureTool>>,
    /// Bi-encoder session pool for concurrent query embedding.
    /// Uses lock-free ArrayQueue for session management (no Mutex serialization).
    pub bi_encoder: Arc<BiEncoderModel>,
    /// Pre-computed tool embeddings for cosine similarity search
    /// Shape: (num_tools, embedding_dim) - computed at startup or loaded from cache
    pub tool_embeddings: Arc<Array2<f32>>,
}

impl AppState {
    /// Initialize application state.
    ///
    /// # Concurrency Strategy
    /// To prevent CPU oversubscription and context switching overhead:
    /// - `permits × intra_threads ≤ physical_cores`
    /// - Default: auto-calculated for optimal throughput
    ///
    /// # Thread Math
    /// - Each request acquires 1 semaphore permit
    /// - Each inference uses `intra_threads` CPU threads
    /// - Total max CPU threads = permits × intra_threads
    ///
    /// Example (12 physical cores):
    /// - permits=6, intra_threads=2 → 12 threads (optimal)
    /// - permits=4, intra_threads=3 → 12 threads (optimal)
    pub fn new(config: Config) -> Result<Self> {
        // Detect available CPU cores
        let logical_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        // Use physical cores only (logical / 2 on hyperthreaded systems)
        // Hyperthreads share execution units, causing contention for CPU-bound inference
        let physical_cores = config
            .pool_size
            .unwrap_or_else(|| (logical_cores / 2).max(1));

        // Auto-calculate permits to prevent CPU oversubscription
        // Formula: permits = physical_cores / intra_threads
        // This ensures: permits × intra_threads ≤ physical_cores
        let intra_threads = config.intra_threads;
        let permits = config.permits.unwrap_or_else(|| {
            let auto_permits = (physical_cores / intra_threads).max(1);
            tracing::info!(
                physical_cores,
                intra_threads,
                auto_permits,
                "Auto-calculated permits: {} cores / {} threads = {} permits",
                physical_cores,
                intra_threads,
                auto_permits
            );
            auto_permits
        });

        // Validate and warn about potential oversubscription
        let total_threads = permits * intra_threads;
        if total_threads > physical_cores {
            tracing::warn!(
                permits,
                intra_threads,
                total_threads,
                physical_cores,
                "⚠️  CPU oversubscription detected! {} permits × {} threads = {} > {} cores. \
                 This may cause context switching overhead. Consider: PERMITS={} or INTRA_THREADS={}",
                permits, intra_threads, total_threads, physical_cores,
                physical_cores / intra_threads, physical_cores / permits
            );
        }

        tracing::info!(
            logical_cores,
            physical_cores,
            intra_threads,
            permits,
            total_threads,
            utilization_pct = (total_threads as f64 / physical_cores as f64 * 100.0) as u32,
            "Concurrency config: {} permits × {} intra_threads = {} threads ({:.0}% of {} cores)",
            permits,
            intra_threads,
            total_threads,
            total_threads as f64 / physical_cores as f64 * 100.0,
            physical_cores
        );

        // Load model pool and tokenizer
        let model =
            RerankerModel::load_pool(&config.model_path, physical_cores, config.intra_threads)?;
        let tokenizer = TokenizerWrapper::load(&config.tokenizer_path, config.max_sequence_length)?;

        // Load tools for semantic routing (optional)
        let (tools, tool_embeddings, bi_encoder) = if let Some(ref tools_path) = config.tools_path {
            tracing::info!(path = %tools_path.display(), "Loading tools for semantic routing");
            let loaded = load_tools_from_file(tools_path)?;
            tracing::info!(count = loaded.len(), "Tools loaded successfully");

            // Try to load embeddings from cache first
            let cache_path = &config.embeddings_cache_path;
            if let Some(cached_embeddings) = try_load_embeddings_cache(cache_path, &loaded)? {
                // Cache hit! Load bi-encoder pool for concurrent query embedding
                tracing::info!(
                    num_tools = loaded.len(),
                    embedding_dim = cached_embeddings.ncols(),
                    "Using cached tool embeddings (skipped batch encoding!)"
                );

                tracing::info!("Loading bi-encoder session pool for query embedding...");
                let bi_encoder = BiEncoderModel::load_pool(
                    &config.bi_encoder_model_path,
                    &config.bi_encoder_tokenizer_path,
                    512,            // BGE-base max length
                    physical_cores, // Pool size matches reranker
                    config.intra_threads,
                )?;

                (loaded, cached_embeddings, bi_encoder)
            } else {
                // Cache miss - load bi-encoder (single session for batch encoding)
                tracing::info!("Loading bi-encoder model for embedding computation...");
                let bi_encoder = BiEncoderModel::load(
                    &config.bi_encoder_model_path,
                    &config.bi_encoder_tokenizer_path,
                    512, // BGE-base max length
                )?;

                tracing::info!("Computing tool embeddings (cache miss)...");
                let inference_views: Vec<String> =
                    loaded.iter().map(|t| t.inference_view.clone()).collect();
                let embeddings = bi_encoder.encode_batch(&inference_views)?;

                tracing::info!(
                    num_tools = loaded.len(),
                    embedding_dim = embeddings.ncols(),
                    "Tool embeddings computed successfully"
                );

                // Save to cache for next startup
                if let Err(e) = save_embeddings_cache(cache_path, &loaded, &embeddings) {
                    tracing::warn!(error = %e, "Failed to save embeddings cache (non-fatal)");
                } else {
                    tracing::info!(path = %cache_path.display(), "Embeddings cache saved");
                }

                // After computing embeddings, reload as pool for runtime queries
                tracing::info!("Reloading bi-encoder as session pool for runtime...");
                let bi_encoder_pool = BiEncoderModel::load_pool(
                    &config.bi_encoder_model_path,
                    &config.bi_encoder_tokenizer_path,
                    512,
                    physical_cores,
                    config.intra_threads,
                )?;

                (loaded, embeddings, bi_encoder_pool)
            }
        } else {
            tracing::info!("No TOOLS_PATH configured, semantic routing disabled");
            // Load bi-encoder pool (may be used for future tool additions)
            let bi_encoder = BiEncoderModel::load_pool(
                &config.bi_encoder_model_path,
                &config.bi_encoder_tokenizer_path,
                512,
                physical_cores,
                config.intra_threads,
            )?;
            (Vec::new(), Array2::zeros((0, 768)), bi_encoder)
        };

        let state = Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            semaphore: Arc::new(Semaphore::new(permits)),
            ready: AtomicBool::new(false),
            config: Arc::new(config),
            tools: Arc::new(tools),
            bi_encoder: Arc::new(bi_encoder),
            tool_embeddings: Arc::new(tool_embeddings),
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
        let (input_ids, attention_mask, _token_type_ids) =
            self.tokenizer.tokenize_pairs(warmup_query, &warmup_docs)?;

        // Run inference (discard results)
        let _ = self.model.inference(input_ids, attention_mask)?;

        tracing::info!("Model warmup completed successfully");
        Ok(())
    }

    /// Check if the service is ready to handle requests.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}

/// Load and atomize tools from a JSON file.
///
/// # Arguments
/// * `path` - Path to the JSON file containing MCP tools
///
/// # Returns
/// A vector of EncapureTool ready for semantic routing
fn load_tools_from_file(path: &Path) -> Result<Vec<EncapureTool>> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::ValidationError(format!(
            "Failed to read tools file '{}': {}",
            path.display(),
            e
        ))
    })?;

    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::ValidationError(format!("Invalid JSON in tools file: {}", e)))?;

    // Extract server name from filename (e.g., "comprehensive_mock_tools.json" -> "comprehensive_mock_tools")
    let server_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    atomize_tools(&json, server_name)
}
