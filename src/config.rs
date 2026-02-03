use std::env;
use std::path::PathBuf;

/// Operating mode for Encapure server.
/// Controls pool_size, permits, and intra_threads settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingMode {
    /// Optimized for single requests with low latency.
    /// pool_size=1, permits=1, intra_threads=8
    Single,
    /// Optimized for concurrent requests with high throughput.
    /// pool_size=10, permits=6, intra_threads=2
    Concurrent,
    /// Use individual environment variable settings.
    Custom,
}

impl OperatingMode {
    pub fn from_env() -> Self {
        match env::var("ENCAPURE_MODE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "single" | "low-latency" | "single-request" => Self::Single,
            "concurrent" | "high-throughput" | "multi" => Self::Concurrent,
            _ => Self::Custom,
        }
    }
}

pub struct Config {
    pub host: String,
    pub port: u16,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub max_sequence_length: usize,
    pub shutdown_timeout_secs: u64,
    /// Optional override for session pool size. If None, uses physical cores.
    pub pool_size: Option<usize>,
    /// Maximum documents per rerank request.
    pub max_documents: usize,
    /// Batch size for internal chunking during inference.
    pub batch_size: usize,
    /// Optional path to tools JSON file for semantic routing.
    pub tools_path: Option<PathBuf>,
    /// Path to bi-encoder ONNX model for fast semantic search.
    pub bi_encoder_model_path: PathBuf,
    /// Path to bi-encoder tokenizer.
    pub bi_encoder_tokenizer_path: PathBuf,
    /// Number of candidates to retrieve in first-stage (bi-encoder) before reranking.
    pub retrieval_candidates: usize,
    /// Number of threads per ONNX session for intra-op parallelism.
    /// Default: 8. Higher values improve single-request latency at the cost of concurrency.
    /// Formula: permits × intra_threads ≤ physical_cores
    pub intra_threads: usize,
    /// Optional override for semaphore permits. If None, auto-calculated as:
    /// physical_cores / intra_threads (ensures no CPU oversubscription)
    pub permits: Option<usize>,
    /// Path to embeddings cache file. Pre-computed embeddings are stored here
    /// to avoid loading the bi-encoder model at runtime.
    pub embeddings_cache_path: PathBuf,
}

impl Config {
    /// Load configuration from environment variables with sensible defaults.
    ///
    /// The `ENCAPURE_MODE` environment variable controls preset configurations:
    /// - `single` / `low-latency`: Optimized for single requests (pool=1, permits=1, intra_threads=8)
    /// - `concurrent` / `high-throughput`: Optimized for concurrent requests (pool=10, permits=6, intra_threads=2)
    /// - Unset or other: Uses individual env vars or defaults
    pub fn from_env() -> anyhow::Result<Self> {
        let mode = OperatingMode::from_env();

        // Determine pool_size, permits, intra_threads based on mode
        let (pool_size, permits, intra_threads) = match mode {
            OperatingMode::Single => {
                // Low latency: single session uses all threads
                (Some(1), Some(1), 8)
            }
            OperatingMode::Concurrent => {
                // High throughput: multiple sessions with fewer threads each
                (Some(10), Some(6), 2)
            }
            OperatingMode::Custom => {
                // Use individual env vars or defaults
                let pool = env::var("POOL_SIZE").ok().and_then(|s| s.parse().ok());
                let perm = env::var("PERMITS").ok().and_then(|s| s.parse().ok());
                let threads = env::var("INTRA_THREADS")
                    .unwrap_or_else(|_| "8".to_string())
                    .parse()?;
                (pool, perm, threads)
            }
        };

        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            model_path: PathBuf::from(
                env::var("MODEL_PATH").unwrap_or_else(|_| "./models/model_int8.onnx".to_string()),
            ),
            tokenizer_path: PathBuf::from(
                env::var("TOKENIZER_PATH")
                    .unwrap_or_else(|_| "./models/tokenizer.json".to_string()),
            ),
            max_sequence_length: env::var("MAX_SEQ_LENGTH")
                .unwrap_or_else(|_| "1024".to_string())
                .parse()?,
            shutdown_timeout_secs: env::var("SHUTDOWN_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            pool_size,
            max_documents: env::var("MAX_DOCUMENTS")
                .unwrap_or_else(|_| "100000".to_string())
                .parse()?,
            batch_size: env::var("BATCH_SIZE")
                .unwrap_or_else(|_| "32".to_string())
                .parse()?,
            tools_path: env::var("TOOLS_PATH").ok().map(PathBuf::from),
            bi_encoder_model_path: PathBuf::from(
                env::var("BI_ENCODER_MODEL_PATH")
                    .unwrap_or_else(|_| "./bi-encoder-model/model_int8.onnx".to_string()),
            ),
            bi_encoder_tokenizer_path: PathBuf::from(
                env::var("BI_ENCODER_TOKENIZER_PATH")
                    .unwrap_or_else(|_| "./bi-encoder-model/tokenizerbiencoder.json".to_string()),
            ),
            retrieval_candidates: env::var("RETRIEVAL_CANDIDATES")
                .unwrap_or_else(|_| "20".to_string())
                .parse()?,
            intra_threads,
            permits,
            embeddings_cache_path: PathBuf::from(
                env::var("EMBEDDINGS_CACHE_PATH")
                    .unwrap_or_else(|_| ".encapure/embeddings.bin".to_string()),
            ),
        })
    }

    /// Returns the operating mode based on current configuration.
    pub fn mode(&self) -> OperatingMode {
        OperatingMode::from_env()
    }
}
