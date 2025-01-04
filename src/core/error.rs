use std::result;
use thiserror::Error;

/// Custom result type for memory operations
pub type Result<T> = result::Result<T, MemoryError>;

/// Errors that can occur during memory operations
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Memory capacity exceeded: {0}")]
    CapacityExceeded(String),

    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        expected: usize,
        actual: usize,
    },

    #[error("Invalid attributes: {0}")]
    InvalidAttributes(String),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(&'static str),

    #[error("Concurrent access error: {0}")]
    ConcurrencyError(String),

    #[error("Memory validation error: {0}")]
    ValidationError(String),
}
