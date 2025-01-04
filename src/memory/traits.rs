use async_trait::async_trait;
use std::time::Duration;

use crate::core::error::Result;
use super::types::{TemporalVector, ContextSummary};

/// Core trait for vector storage operations
#[async_trait]
pub trait VectorStorage: Send + Sync {
    /// Insert a new memory into storage
    async fn insert_memory(&self, memory: TemporalVector) -> Result<()>;

    /// Retrieve a memory by its ID
    async fn get_memory(&self, id: &str) -> Result<Option<TemporalVector>>;

    /// Search for memories by context
    async fn search_by_context(&self, context: &str, limit: usize) -> Result<Vec<TemporalVector>>;

    /// Get memories above an importance threshold
    async fn get_important_memories(&self, threshold: f32) -> Result<Vec<TemporalVector>>;

    /// Get memories related to a given memory ID
    async fn get_related_memories(&self, id: &str) -> Result<Vec<TemporalVector>>;

    /// Apply decay to all memories based on time duration
    async fn apply_decay(&self, duration: Duration) -> Result<()>;

    /// Consolidate memories within a time window
    async fn consolidate_memories(&self, time_window: Duration) -> Result<()>;

    /// Get a summary of memories in a context
    async fn get_context_summary(&self, context: &str) -> Result<Option<ContextSummary>>;
}
