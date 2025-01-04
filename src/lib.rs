//! Vector Store - A high-performance vector storage implementation
//! 
//! This crate provides a sophisticated vector storage system with temporal memory management,
//! designed for AI applications. It features memory consolidation, decay simulation, and
//! relationship tracking.

// Core modules
pub mod config;
pub mod core;
pub mod memory;
pub mod server;
pub mod storage;
pub mod telemetry;
pub mod utils;

// Re-export commonly used items
pub use config::Config;
pub use core::{
    error::{Result, MemoryError},
    config::MemoryConfig,
    logging,
};
pub use memory::{
    temporal::MemoryStorage,
    types::{MemoryAttributes, TemporalVector, Vector, MemoryStats},
};
pub use server::Server;
pub use storage::{
    metrics::CosineDistance,
    hnsw::TemporalHNSW,
    persistence::MemoryBackend,
};
pub use telemetry::{init_telemetry, shutdown_telemetry};

/// Initialize ChronoMind with default configuration
pub fn init() -> Result<Server> {
    init_with_config(Config::default())
}

/// Initialize ChronoMind with custom configuration
pub fn init_with_config(config: Config) -> Result<Server> {
    let backend = MemoryBackend::new(MemoryConfig::default());
    Ok(Server::new(config, backend))
}

/// Save a memory to the store
pub async fn save_memory(store: &mut Server, memory: TemporalVector) -> Result<()> {
    store.save_memory(memory).await
}

/// Get a memory by ID
pub async fn get_memory(store: &Server, id: &str) -> Result<Option<TemporalVector>> {
    store.get_memory(id).await
}

/// Search for similar memories
pub async fn search_similar(
    store: &Server,
    query: &[f32],
    limit: usize,
) -> Result<Vec<(TemporalVector, f32)>> {
    store.search_similar(query, limit).await
}

/// Update memory decay
pub async fn update_memory_decay(store: &mut Server) -> Result<()> {
    store.update_memory_decay().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let server = init();
        assert!(server.is_ok());
    }

    #[test]
    fn test_custom_config() {
        let config = Config::default();
        let server = init_with_config(config);
        assert!(server.is_ok());
    }
}
