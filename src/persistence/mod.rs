//! Persistence layer for caching pre-computed embeddings.
//!
//! This module provides efficient storage and retrieval of tool embeddings,
//! eliminating the need to keep the bi-encoder model in memory at runtime.

use crate::error::{AppError, Result};
use crate::ingestion::EncapureTool;
use ndarray::Array2;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Cache file format version. Increment when format changes.
const CACHE_VERSION: u32 = 1;

/// Magic bytes to identify valid cache files.
const CACHE_MAGIC: &[u8; 8] = b"ENCAPURE";

/// Cached embeddings with metadata for validation.
pub struct EmbeddingsCache {
    /// Format version for compatibility checking
    pub version: u32,
    /// SHA256 hash of tools to detect changes
    pub tools_hash: [u8; 32],
    /// Number of tools
    pub num_tools: usize,
    /// Embedding dimension (768 for BGE-base)
    pub embedding_dim: usize,
    /// Pre-computed embeddings matrix (num_tools × embedding_dim)
    pub embeddings: Array2<f32>,
}

impl EmbeddingsCache {
    /// Compute SHA256 hash of tools for cache invalidation.
    ///
    /// Hash is based on tool names and inference_views to detect any changes.
    pub fn compute_tools_hash(tools: &[EncapureTool]) -> [u8; 32] {
        let mut hasher = Sha256::new();

        for tool in tools {
            hasher.update(tool.name.as_bytes());
            hasher.update(b"|");
            hasher.update(tool.inference_view.as_bytes());
            hasher.update(b"\n");
        }

        hasher.finalize().into()
    }

    /// Create a new cache from computed embeddings.
    pub fn new(tools: &[EncapureTool], embeddings: Array2<f32>) -> Self {
        Self {
            version: CACHE_VERSION,
            tools_hash: Self::compute_tools_hash(tools),
            num_tools: tools.len(),
            embedding_dim: embeddings.ncols(),
            embeddings,
        }
    }

    /// Save cache to binary file.
    ///
    /// File format:
    /// - 8 bytes: magic "ENCAPURE"
    /// - 4 bytes: version (u32 LE)
    /// - 32 bytes: tools_hash
    /// - 8 bytes: num_tools (u64 LE)
    /// - 8 bytes: embedding_dim (u64 LE)
    /// - N bytes: embeddings data (f32 LE, row-major)
    pub fn save(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::ValidationError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        let file = File::create(path).map_err(|e| {
            AppError::ValidationError(format!("Failed to create cache file: {}", e))
        })?;
        let mut writer = BufWriter::new(file);

        // Write header
        writer.write_all(CACHE_MAGIC).map_err(|e| {
            AppError::ValidationError(format!("Failed to write cache magic: {}", e))
        })?;

        writer.write_all(&self.version.to_le_bytes()).map_err(|e| {
            AppError::ValidationError(format!("Failed to write cache version: {}", e))
        })?;

        writer.write_all(&self.tools_hash).map_err(|e| {
            AppError::ValidationError(format!("Failed to write tools hash: {}", e))
        })?;

        writer
            .write_all(&(self.num_tools as u64).to_le_bytes())
            .map_err(|e| {
                AppError::ValidationError(format!("Failed to write num_tools: {}", e))
            })?;

        writer
            .write_all(&(self.embedding_dim as u64).to_le_bytes())
            .map_err(|e| {
                AppError::ValidationError(format!("Failed to write embedding_dim: {}", e))
            })?;

        // Write embeddings data
        let data = self.embeddings.as_slice().ok_or_else(|| {
            AppError::ValidationError("Embeddings array not contiguous".to_string())
        })?;

        for &val in data {
            writer.write_all(&val.to_le_bytes()).map_err(|e| {
                AppError::ValidationError(format!("Failed to write embedding data: {}", e))
            })?;
        }

        writer.flush().map_err(|e| {
            AppError::ValidationError(format!("Failed to flush cache file: {}", e))
        })?;

        tracing::info!(
            path = %path.display(),
            num_tools = self.num_tools,
            embedding_dim = self.embedding_dim,
            size_bytes = 8 + 4 + 32 + 8 + 8 + (self.num_tools * self.embedding_dim * 4),
            "Embeddings cache saved"
        );

        Ok(())
    }

    /// Load cache from binary file.
    ///
    /// Returns None if file doesn't exist or is invalid.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            tracing::debug!(path = %path.display(), "Cache file does not exist");
            return Ok(None);
        }

        let file = File::open(path).map_err(|e| {
            AppError::ValidationError(format!("Failed to open cache file: {}", e))
        })?;
        let mut reader = BufReader::new(file);

        // Read and validate magic
        let mut magic = [0u8; 8];
        if reader.read_exact(&mut magic).is_err() || &magic != CACHE_MAGIC {
            tracing::warn!(path = %path.display(), "Invalid cache magic, ignoring");
            return Ok(None);
        }

        // Read version
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes).map_err(|e| {
            AppError::ValidationError(format!("Failed to read cache version: {}", e))
        })?;
        let version = u32::from_le_bytes(version_bytes);

        if version != CACHE_VERSION {
            tracing::warn!(
                path = %path.display(),
                cache_version = version,
                expected_version = CACHE_VERSION,
                "Cache version mismatch, ignoring"
            );
            return Ok(None);
        }

        // Read tools hash
        let mut tools_hash = [0u8; 32];
        reader.read_exact(&mut tools_hash).map_err(|e| {
            AppError::ValidationError(format!("Failed to read tools hash: {}", e))
        })?;

        // Read dimensions
        let mut num_tools_bytes = [0u8; 8];
        reader.read_exact(&mut num_tools_bytes).map_err(|e| {
            AppError::ValidationError(format!("Failed to read num_tools: {}", e))
        })?;
        let num_tools = u64::from_le_bytes(num_tools_bytes) as usize;

        let mut embedding_dim_bytes = [0u8; 8];
        reader.read_exact(&mut embedding_dim_bytes).map_err(|e| {
            AppError::ValidationError(format!("Failed to read embedding_dim: {}", e))
        })?;
        let embedding_dim = u64::from_le_bytes(embedding_dim_bytes) as usize;

        // Read embeddings data
        let total_floats = num_tools * embedding_dim;
        let mut data = vec![0f32; total_floats];

        for val in &mut data {
            let mut bytes = [0u8; 4];
            reader.read_exact(&mut bytes).map_err(|e| {
                AppError::ValidationError(format!("Failed to read embedding data: {}", e))
            })?;
            *val = f32::from_le_bytes(bytes);
        }

        let embeddings = Array2::from_shape_vec((num_tools, embedding_dim), data).map_err(|e| {
            AppError::ValidationError(format!("Failed to reshape embeddings: {}", e))
        })?;

        tracing::info!(
            path = %path.display(),
            num_tools,
            embedding_dim,
            "Embeddings cache loaded"
        );

        Ok(Some(Self {
            version,
            tools_hash,
            num_tools,
            embedding_dim,
            embeddings,
        }))
    }

    /// Check if cache is valid for the given tools.
    pub fn is_valid_for(&self, tools: &[EncapureTool]) -> bool {
        let current_hash = Self::compute_tools_hash(tools);
        self.tools_hash == current_hash && self.num_tools == tools.len()
    }
}

/// Try to load embeddings from cache, validating against current tools.
///
/// Returns Some(embeddings) if cache is valid, None if cache miss.
pub fn try_load_embeddings_cache(
    cache_path: &Path,
    tools: &[EncapureTool],
) -> Result<Option<Array2<f32>>> {
    match EmbeddingsCache::load(cache_path)? {
        Some(cache) if cache.is_valid_for(tools) => {
            tracing::info!("Using cached embeddings (cache hit)");
            Ok(Some(cache.embeddings))
        }
        Some(_) => {
            tracing::info!("Cache invalid (tools changed), will recompute");
            Ok(None)
        }
        None => {
            tracing::info!("No cache found, will compute embeddings");
            Ok(None)
        }
    }
}

/// Save computed embeddings to cache.
pub fn save_embeddings_cache(
    cache_path: &Path,
    tools: &[EncapureTool],
    embeddings: &Array2<f32>,
) -> Result<()> {
    let cache = EmbeddingsCache::new(tools, embeddings.clone());
    cache.save(cache_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn make_test_tool(name: &str, desc: &str) -> EncapureTool {
        EncapureTool {
            name: name.to_string(),
            server_origin: "test".to_string(),
            inference_view: format!("TOOL: {} | FUNC: {}", name, desc),
            raw_definition: json!({"name": name}),
        }
    }

    #[test]
    fn test_hash_changes_with_tools() {
        let tools1 = vec![make_test_tool("tool1", "desc1")];
        let tools2 = vec![make_test_tool("tool2", "desc2")];

        let hash1 = EmbeddingsCache::compute_tools_hash(&tools1);
        let hash2 = EmbeddingsCache::compute_tools_hash(&tools2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let cache_path = dir.path().join("test_cache.bin");

        let tools = vec![
            make_test_tool("tool1", "desc1"),
            make_test_tool("tool2", "desc2"),
        ];
        let embeddings = Array2::from_shape_vec((2, 4), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
            .unwrap();

        // Save
        let cache = EmbeddingsCache::new(&tools, embeddings.clone());
        cache.save(&cache_path).unwrap();

        // Load
        let loaded = EmbeddingsCache::load(&cache_path).unwrap().unwrap();

        assert_eq!(loaded.num_tools, 2);
        assert_eq!(loaded.embedding_dim, 4);
        assert_eq!(loaded.embeddings, embeddings);
        assert!(loaded.is_valid_for(&tools));
    }

    #[test]
    fn test_cache_invalidation() {
        let dir = tempdir().unwrap();
        let cache_path = dir.path().join("test_cache.bin");

        let tools1 = vec![make_test_tool("tool1", "desc1")];
        let tools2 = vec![make_test_tool("tool2", "desc2")];
        let embeddings = Array2::from_shape_vec((1, 4), vec![1.0, 2.0, 3.0, 4.0]).unwrap();

        // Save with tools1
        let cache = EmbeddingsCache::new(&tools1, embeddings);
        cache.save(&cache_path).unwrap();

        // Load and check - should be invalid for tools2
        let loaded = EmbeddingsCache::load(&cache_path).unwrap().unwrap();
        assert!(loaded.is_valid_for(&tools1));
        assert!(!loaded.is_valid_for(&tools2));
    }
}
