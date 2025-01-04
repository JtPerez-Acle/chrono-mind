use async_trait::async_trait;
use std::path::PathBuf;
use crate::{
    core::error::Result,
    memory::types::{TemporalVector, MemoryStats},
};

/// Trait for storage backend implementations
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Initialize the storage backend
    async fn init(&mut self) -> Result<()>;

    /// Save a memory to storage
    async fn save(&mut self, memory: &TemporalVector) -> Result<()>;

    /// Load a memory by ID
    async fn load(&self, id: &str) -> Result<Option<TemporalVector>>;

    /// Delete a memory by ID
    async fn delete(&mut self, id: &str) -> Result<()>;

    /// List all memory IDs
    async fn list_ids(&self) -> Result<Vec<String>>;

    /// Get storage statistics
    async fn get_stats(&self) -> Result<MemoryStats>;

    /// Create a backup of the storage
    async fn backup(&self, path: PathBuf) -> Result<()>;

    /// Restore from a backup
    async fn restore(&mut self, path: PathBuf) -> Result<()>;
}

/// In-memory storage backend implementation
pub struct MemoryBackend {
    memories: std::collections::HashMap<String, TemporalVector>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            memories: std::collections::HashMap::new(),
        }
    }
}

#[async_trait]
impl StorageBackend for MemoryBackend {
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    async fn save(&mut self, memory: &TemporalVector) -> Result<()> {
        self.memories.insert(memory.vector.id.clone(), memory.clone());
        Ok(())
    }

    async fn load(&self, id: &str) -> Result<Option<TemporalVector>> {
        Ok(self.memories.get(id).cloned())
    }

    async fn delete(&mut self, id: &str) -> Result<()> {
        self.memories.remove(id);
        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<String>> {
        Ok(self.memories.keys().cloned().collect())
    }

    async fn get_stats(&self) -> Result<MemoryStats> {
        let total_memories = self.memories.len();
        let mut total_importance = 0.0;
        let mut context_counts = std::collections::HashMap::new();
        let mut relationship_counts = std::collections::HashMap::new();

        for memory in self.memories.values() {
            total_importance += memory.attributes.importance;
            *context_counts.entry(memory.attributes.context.clone()).or_insert(0) += 1;
            
            for rel in &memory.attributes.relationships {
                *relationship_counts.entry(rel.clone()).or_insert(0) += 1;
            }
        }

        Ok(MemoryStats {
            total_memories,
            capacity_used: 100.0, // In-memory has no fixed capacity
            average_importance: if total_memories > 0 {
                total_importance / total_memories as f32
            } else {
                0.0
            },
            context_distribution: context_counts,
            most_connected_memories: relationship_counts.into_iter()
                .collect::<Vec<_>>()
                .into_iter()
                .take(10)
                .collect(),
        })
    }

    async fn backup(&self, path: PathBuf) -> Result<()> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer(file, &self.memories)?;
        Ok(())
    }

    async fn restore(&mut self, path: PathBuf) -> Result<()> {
        let file = std::fs::File::open(path)?;
        self.memories = serde_json::from_reader(file)?;
        Ok(())
    }
}
