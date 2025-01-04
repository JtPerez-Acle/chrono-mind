use thiserror::Error;

/// Result type alias for operations that may return a MemoryError
pub type Result<T> = std::result::Result<T, MemoryError>;

/// Error types that can occur during memory operations
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Invalid vector dimensions: got {got}, expected {expected}")]
    InvalidDimensions {
        got: usize,
        expected: usize,
    },
    
    #[error("Invalid vector data: {0}")]
    InvalidVectorData(String),
    
    #[error("Invalid importance value: {0}")]
    InvalidImportance(f32),
    
    #[error("Invalid attributes: {0}")]
    InvalidAttributes(String),
    
    #[error("Storage is full: {0}")]
    StorageFull(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Task error: {0}")]
    TaskError(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<tokio::task::JoinError> for MemoryError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::TaskError(err.to_string())
    }
}
