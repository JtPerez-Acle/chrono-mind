//! Error types for all fallible ChronoMind operations.

use thiserror::Error;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The error type for all ChronoMind operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A vector's dimensionality does not match the store configuration.
    #[error("invalid dimensions: got {got}, expected {expected}")]
    InvalidDimensions {
        /// Dimensions of the offending vector.
        got: usize,
        /// Dimensions the store was configured for.
        expected: usize,
    },

    /// A vector contains invalid data (empty, NaN, or infinite components).
    #[error("invalid vector data: {0}")]
    InvalidVector(String),

    /// An importance value is outside the `[0.0, 1.0]` range.
    #[error("invalid importance value {0}: must be within [0.0, 1.0]")]
    InvalidImportance(f32),

    /// No memory exists with the given id.
    #[error("memory not found: {0}")]
    NotFound(String),

    /// The store has reached its configured `max_memories` capacity.
    #[error("store is at capacity ({0} memories)")]
    CapacityExceeded(usize),

    /// The configuration failed validation.
    #[error("invalid configuration: {0}")]
    Config(String),

    /// An underlying IO operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A snapshot could not be encoded or decoded.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// A snapshot file is not a ChronoMind snapshot or uses an unsupported format version.
    #[error("invalid snapshot: {0}")]
    InvalidSnapshot(String),
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
