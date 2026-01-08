use crate::error::{AppError, Result};
use ndarray::Array2;
use std::path::Path;
use tokenizers::Tokenizer;

pub struct TokenizerWrapper {
    tokenizer: Tokenizer,
    max_length: usize,
}

impl TokenizerWrapper {
    pub fn load(tokenizer_path: &Path, max_length: usize) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| AppError::TokenizationError(e.to_string()))?;

        tracing::info!(
            path = %tokenizer_path.display(),
            max_length,
            "Tokenizer loaded successfully"
        );

        Ok(Self {
            tokenizer,
            max_length,
        })
    }

    /// Tokenize query-document pairs for reranking.
    /// Returns (input_ids, attention_mask, token_type_ids) as Array2<i64>.
    pub fn tokenize_pairs(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<(Array2<i64>, Array2<i64>, Array2<i64>)> {
        if documents.is_empty() {
            return Err(AppError::ValidationError(
                "Documents list cannot be empty".to_string(),
            ));
        }

        let batch_size = documents.len();

        // Encode each (query, document) pair
        let mut encodings = Vec::with_capacity(batch_size);
        for doc in documents {
            let encoding = self
                .tokenizer
                .encode((query, doc.as_str()), true)
                .map_err(|e| AppError::TokenizationError(e.to_string()))?;
            encodings.push(encoding);
        }

        // Find max length in batch (for minimal padding), capped at max_sequence_length
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(self.max_length))
            .max()
            .unwrap_or(0);

        // Allocate arrays
        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));
        let mut token_type_ids = Array2::<i64>::zeros((batch_size, max_len));

        // Fill arrays
        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let types = encoding.get_type_ids();

            let len = ids.len().min(max_len);

            for j in 0..len {
                input_ids[[i, j]] = ids[j] as i64;
                attention_mask[[i, j]] = mask[j] as i64;
                token_type_ids[[i, j]] = types[j] as i64;
            }
        }

        Ok((input_ids, attention_mask, token_type_ids))
    }
}
