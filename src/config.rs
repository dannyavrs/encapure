use std::env;
use std::path::PathBuf;

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
    /// Higher = faster single-request batches, but more CPU contention under load.
    /// Default: 4 (good for batch processing in /search)
    pub intra_threads: usize,
}

impl Config {
    /// Load configuration from environment variables with sensible defaults.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            model_path: PathBuf::from(
                env::var("MODEL_PATH")
                    .unwrap_or_else(|_| "./models/model_int8.onnx".to_string()),
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
            pool_size: env::var("POOL_SIZE").ok().and_then(|s| s.parse().ok()),
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
                .unwrap_or_else(|_| "8".to_string())
                .parse()?,
            intra_threads: env::var("INTRA_THREADS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()?,
        })
    }
}
