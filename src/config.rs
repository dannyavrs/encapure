use std::env;
use std::path::PathBuf;

pub struct Config {
    pub host: String,
    pub port: u16,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub max_sequence_length: usize,
    pub shutdown_timeout_secs: u64,
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
                    .unwrap_or_else(|_| "./models/model_quantized.onnx".to_string()),
            ),
            tokenizer_path: PathBuf::from(
                env::var("TOKENIZER_PATH")
                    .unwrap_or_else(|_| "./models/tokenizer.json".to_string()),
            ),
            max_sequence_length: env::var("MAX_SEQ_LENGTH")
                .unwrap_or_else(|_| "8192".to_string())
                .parse()?,
            shutdown_timeout_secs: env::var("SHUTDOWN_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
        })
    }
}
