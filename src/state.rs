use crate::config::Config;
use crate::error::{AppError, Result};
use crate::inference::{BiEncoderModel, RerankerModel, TokenizerWrapper};
use crate::ingestion::{atomize_tools, EncapureTool};
use ndarray::Array2;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    /// Bi-encoder model for fast first-stage retrieval (Mutex because encode needs &mut self)
    pub bi_encoder: Arc<Mutex<BiEncoderModel>>,
    /// Pre-computed tool embeddings for cosine similarity search
    /// Shape: (num_tools, embedding_dim) - computed at startup
    pub tool_embeddings: Arc<Array2<f32>>,
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
        let model = RerankerModel::load_pool(&config.model_path, num_cores, config.intra_threads)?;
        let tokenizer =
            TokenizerWrapper::load(&config.tokenizer_path, config.max_sequence_length)?;

        // Load bi-encoder for fast first-stage retrieval
        tracing::info!("Loading bi-encoder model for fast retrieval...");
        let mut bi_encoder = BiEncoderModel::load(
            &config.bi_encoder_model_path,
            &config.bi_encoder_tokenizer_path,
            512, // BGE-base max length
        )?;

        // Load tools for semantic routing (optional)
        let (tools, tool_embeddings) = if let Some(ref tools_path) = config.tools_path {
            tracing::info!(path = %tools_path.display(), "Loading tools for semantic routing");
            let loaded = load_tools_from_file(tools_path)?;
            tracing::info!(count = loaded.len(), "Tools loaded successfully");

            // Pre-compute embeddings for all tools at startup
            tracing::info!("Pre-computing tool embeddings...");
            let inference_views: Vec<String> =
                loaded.iter().map(|t| t.inference_view.clone()).collect();
            let embeddings = bi_encoder.encode_batch(&inference_views)?;
            tracing::info!(
                num_tools = loaded.len(),
                embedding_dim = embeddings.ncols(),
                "Tool embeddings computed successfully"
            );

            (loaded, embeddings)
        } else {
            tracing::info!("No TOOLS_PATH configured, semantic routing disabled");
            (Vec::new(), Array2::zeros((0, 768)))
        };

        let state = Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            semaphore: Arc::new(Semaphore::new(num_cores)),
            ready: AtomicBool::new(false),
            config: Arc::new(config),
            tools: Arc::new(tools),
            bi_encoder: Arc::new(Mutex::new(bi_encoder)),
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
        AppError::ValidationError(format!("Failed to read tools file '{}': {}", path.display(), e))
    })?;

    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        AppError::ValidationError(format!("Invalid JSON in tools file: {}", e))
    })?;

    // Extract server name from filename (e.g., "comprehensive_mock_tools.json" -> "comprehensive_mock_tools")
    let server_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    atomize_tools(&json, server_name)
}
