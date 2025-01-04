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

// Re-exports of commonly used types
pub use core::config::MemoryConfig;
pub use core::error::{MemoryError, Result};
pub use memory::types::{MemoryAttributes, TemporalVector, Vector};
pub use storage::{
    metrics::{CosineDistance, DistanceMetric},
    hnsw::{HNSWConfig, TemporalHNSW},
};

/// Initialize the vector store with default configuration
pub fn init() -> Result<()> {
    init_with_config(MemoryConfig::default())
}

/// Initialize the vector store with custom configuration
pub fn init_with_config(config: MemoryConfig) -> Result<()> {
    config.validate().map_err(MemoryError::ConfigError)?;
    
    // Initialize logging
    core::logging::init_logging();
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        assert!(init().is_ok());
    }

    #[test]
    fn test_custom_config() {
        let config = MemoryConfig::default();
        assert!(init_with_config(config).is_ok());
    }
}
