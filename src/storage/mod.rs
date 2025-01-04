//! Storage implementations for vector and memory persistence
//! 
//! This module provides different storage backends and metrics implementations
//! for the vector memory system.

use crate::core::error::Result;
use async_trait::async_trait;

pub mod metrics;
pub mod persistence;
pub mod hnsw;

pub use metrics::DistanceMetric;
pub use persistence::StorageBackend;
pub use hnsw::{HNSWConfig, TemporalHNSW};

/// A vector with its identifier
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    /// Unique identifier for the vector
    pub id: String,
    /// Vector data
    pub data: Vec<f32>,
    /// Optional metadata for the vector
    pub metadata: Option<serde_json::Value>,
}

/// Trait for vector storage implementations
#[async_trait]
pub trait VectorStorage: Send + Sync {
    /// Insert a vector into storage
    async fn insert(&mut self, vector: Vector) -> Result<()>;

    /// Search for similar vectors
    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(Vector, f32)>>;

    /// Delete a vector by ID
    async fn delete(&mut self, id: &str) -> Result<()>;

    /// Get a vector by ID
    async fn get(&self, id: &str) -> Result<Option<Vector>>;

    /// Get the number of vectors in storage
    async fn len(&self) -> Result<usize>;

    /// Check if storage is empty
    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
}
