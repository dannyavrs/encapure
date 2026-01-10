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
                    .unwrap_or_else(|_| "./models/model_quint8_avx2.onnx".to_string()),
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
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
        })
    }
}
