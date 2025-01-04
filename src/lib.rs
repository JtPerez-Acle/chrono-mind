//! Vector Store - A high-performance vector storage implementation
//! 
//! This crate provides a sophisticated vector storage system with temporal memory management,
//! designed for AI applications. It features memory consolidation, decay simulation, and
//! relationship tracking.

// Core modules
pub mod core;
pub mod memory;
pub mod storage;
pub mod utils;

pub use crate::{
    core::{
        config::MemoryConfig,
        error::{MemoryError, Result},
    },
    memory::{
        temporal::MemoryStorage,
        types::{MemoryAttributes, TemporalVector, Vector, MemoryStats},
    },
    storage::{
        metrics::CosineDistance,
        hnsw::TemporalHNSW,
    },
};

use std::sync::Arc;

/// Initialize the vector store with default configuration
pub fn init() -> Result<MemoryStorage> {
    init_with_config(MemoryConfig::default())
}

/// Initialize the vector store with custom configuration
pub fn init_with_config(config: MemoryConfig) -> Result<MemoryStorage> {
    // Validate configuration
    if let Err(e) = config.validate() {
        return Err(MemoryError::ConfigError(e.to_string()));
    }
    
    // Initialize storage
    let metric = Arc::new(CosineDistance::new());
    let memory_config = config;
    Ok(MemoryStorage::new(memory_config, metric))
}

/// Initialize memory store with custom distance metric
pub async fn init_memory_store(
    distance_metric: Arc<dyn crate::storage::metrics::DistanceMetric + Send + Sync>,
) -> Result<MemoryStorage> {
    let config = MemoryConfig::default();
    config.validate()?;
    let memory_config = config;
    Ok(MemoryStorage::new(memory_config, distance_metric))
}

/// Save a memory to the store
pub async fn save_memory(store: &mut MemoryStorage, memory: TemporalVector) -> Result<()> {
    store.save_memory(memory).await
}

/// Get a memory from the store by ID
pub async fn get_memory(store: &MemoryStorage, id: &str) -> Result<Option<TemporalVector>> {
    store.get_memory(id).await
}

/// Search for similar memories
pub async fn search_similar(
    store: &MemoryStorage,
    query: &[f32],
    limit: usize,
) -> Result<Vec<(TemporalVector, f32)>> {
    store.search_similar(query, limit).await
}

/// Update memory decay
pub async fn update_memory_decay(store: &mut MemoryStorage) -> Result<()> {
    store.update_memory_decay().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::MemoryConfig;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }

    #[test]
    fn test_custom_config() {
        let config = MemoryConfig::default();
        assert!(init_with_config(config).is_ok());
    }
}
