use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use opentelemetry::trace::Tracer;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::{
    core::{
        config::MemoryConfig,
        error::{MemoryError, Result},
    },
    memory::{
        traits::VectorStorage,
        types::{MemoryStats, TemporalVector, ContextSummary},
    },
    utils::validation::{validate_vector_data, validate_vector_dimensions},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct PersistentStore {
    pub memories: HashMap<String, TemporalVector>,
    pub config: MemoryConfig,
}

impl Default for PersistentStore {
    fn default() -> Self {
        Self {
            memories: HashMap::new(),
            config: MemoryConfig::default(),
        }
    }
}

impl PersistentStore {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            memories: HashMap::new(),
            config,
        }
    }

    pub fn save_memory(&mut self, memory: TemporalVector) -> Result<()> {
        validate_vector_dimensions(&memory.vector.data, &self.config)?;
        validate_vector_data(&memory.vector.data)?;
        self.memories.insert(memory.vector.id.clone(), memory);
        Ok(())
    }

    pub fn remove_memory(&mut self, id: &str) -> Option<TemporalVector> {
        self.memories.remove(id)
    }

    pub fn list_memories(&self) -> Vec<&TemporalVector> {
        self.memories.values().collect()
    }

    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    pub fn load_from_file(&mut self, path: &PathBuf) -> Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        *self = serde_json::from_reader(reader)?;
        Ok(())
    }
}

/// In-memory storage backend implementation
#[derive(Debug)]
pub struct MemoryBackend {
    store: Arc<RwLock<PersistentStore>>,
    tracer: opentelemetry::global::BoxedTracer,
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self {
            store: Arc::new(RwLock::new(PersistentStore::default())),
            tracer: opentelemetry::global::tracer("memory_backend"),
        }
    }
}

impl MemoryBackend {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            store: Arc::new(RwLock::new(PersistentStore::new(config))),
            tracer: opentelemetry::global::tracer("memory_backend"),
        }
    }

    async fn save(&mut self, memory: &TemporalVector) -> Result<()> {
        let _span = self.tracer.start("save_memory");
        
        validate_vector_dimensions(&memory.vector.data, &self.store.read().await.config)?;
        validate_vector_data(&memory.vector.data)?;

        info!(memory_id = %memory.vector.id, "Saving memory vector");
        self.store.write().await.save_memory(memory.clone())
    }

    async fn get(&self, id: &str) -> Result<Option<TemporalVector>> {
        let _span = self.tracer.start("get_memory");
        Ok(self.store.read().await.memories.get(id).cloned())
    }

    async fn remove(&mut self, id: &str) -> Option<TemporalVector> {
        let _span = self.tracer.start("remove_memory");
        self.store.write().await.remove_memory(id)
    }

    async fn list(&self) -> Vec<TemporalVector> {
        let _span = self.tracer.start("list_memories");
        self.store.read().await.list_memories().into_iter().cloned().collect()
    }

    async fn count(&self) -> usize {
        let _span = self.tracer.start("count_memories");
        self.store.read().await.memory_count()
    }

    pub async fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let _span = self.tracer.start("save_to_file");
        self.store.read().await.save_to_file(path)
    }

    pub async fn load_from_file(&mut self, path: &PathBuf) -> Result<()> {
        let _span = self.tracer.start("load_from_file");
        self.store.write().await.load_from_file(path)
    }
}

#[async_trait::async_trait]
impl VectorStorage for MemoryBackend {
    async fn insert_memory(&self, memory: TemporalVector) -> Result<()> {
        let mut store = self.store.write().await;
        store.save_memory(memory)
    }

    async fn get_memory(&self, id: &str) -> Result<Option<TemporalVector>> {
        let store = self.store.read().await;
        Ok(store.memories.get(id).cloned())
    }

    async fn search_by_context(&self, context: &str, limit: usize) -> Result<Vec<TemporalVector>> {
        let store = self.store.read().await;
        let mut memories: Vec<_> = store.memories.values()
            .filter(|m| m.attributes.context == context)
            .cloned()
            .collect();
        memories.sort_by(|a, b| b.attributes.importance.partial_cmp(&a.attributes.importance).unwrap());
        Ok(memories.into_iter().take(limit).collect())
    }

    async fn get_important_memories(&self, threshold: f32) -> Result<Vec<TemporalVector>> {
        let store = self.store.read().await;
        Ok(store.memories.values()
            .filter(|m| m.attributes.importance >= threshold)
            .cloned()
            .collect())
    }

    async fn get_related_memories(&self, id: &str) -> Result<Vec<TemporalVector>> {
        let store = self.store.read().await;
        if let Some(memory) = store.memories.get(id) {
            Ok(store.memories.values()
                .filter(|m| m.vector.id != id && m.attributes.context == memory.attributes.context)
                .cloned()
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn apply_decay(&self, duration: Duration) -> Result<()> {
        let mut store = self.store.write().await;
        let decay_rate = store.config.base_decay_rate;
        for memory in store.memories.values_mut() {
            memory.attributes.importance *= (-duration.as_secs_f32() * decay_rate).exp();
        }
        Ok(())
    }

    async fn consolidate_memories(&self, time_window: Duration) -> Result<()> {
        let mut store = self.store.write().await;
        let now = std::time::SystemTime::now();
        let mut to_remove = Vec::new();
        
        for (id, memory) in store.memories.iter() {
            if memory.attributes.importance < store.config.min_importance {
                if let Ok(age) = now.duration_since(memory.created_at) {
                    if age > time_window {
                        to_remove.push(id.clone());
                    }
                }
            }
        }
        
        for id in to_remove {
            store.memories.remove(&id);
        }
        
        Ok(())
    }

    async fn get_context_summary(&self, context: &str) -> Result<Option<ContextSummary>> {
        let store = self.store.read().await;
        let memories: Vec<_> = store.memories.values()
            .filter(|m| m.attributes.context == context)
            .collect();
            
        if memories.is_empty() {
            return Ok(None);
        }
        
        let count = memories.len();
        let importance = memories.iter()
            .map(|m| m.attributes.importance)
            .sum::<f32>() / count as f32;
            
        Ok(Some(ContextSummary {
            context: context.to_string(),
            memory_count: count,
            importance,
        }))
    }
}

/// Trait for storage backend implementations
#[async_trait::async_trait]
pub trait StorageBackend {
    /// Initialize the storage backend
    async fn init(&mut self) -> Result<()>;

    /// Save a memory vector
    async fn save(&mut self, memory: &TemporalVector) -> Result<()>;

    /// Load a memory vector by ID
    async fn load(&self, id: &str) -> Result<Option<TemporalVector>>;

    /// Delete a memory vector by ID
    async fn delete(&mut self, id: &str) -> Result<()>;

    /// List all memory vector IDs
    async fn list_ids(&self) -> Result<Vec<String>>;

    /// Get memory storage statistics
    async fn get_stats(&self) -> Result<MemoryStats>;

    /// Backup memory store to file
    async fn backup(&self, path: PathBuf) -> Result<()>;

    /// Restore memory store from file
    async fn restore(&mut self, path: PathBuf) -> Result<()>;
}

#[async_trait::async_trait]
impl StorageBackend for MemoryBackend {
    #[tracing::instrument(skip(self))]
    async fn init(&mut self) -> Result<()> {
        info!("Initializing memory backend");
        Ok(())
    }

    #[tracing::instrument(skip(self, memory), fields(memory_id = %memory.vector.id))]
    async fn save(&mut self, memory: &TemporalVector) -> Result<()> {
        self.save(memory).await
    }

    #[tracing::instrument(skip(self))]
    async fn load(&self, id: &str) -> Result<Option<TemporalVector>> {
        self.get(id).await
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&mut self, id: &str) -> Result<()> {
        if self.remove(id).await.is_some() {
            info!(memory_id = %id, "Memory vector deleted successfully");
            Ok(())
        } else {
            warn!(memory_id = %id, "Memory vector not found");
            Err(MemoryError::NotFound(id.to_string()))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn list_ids(&self) -> Result<Vec<String>> {
        Ok(self.list().await.into_iter().map(|m| m.vector.id).collect())
    }

    #[tracing::instrument(skip(self))]
    async fn get_stats(&self) -> Result<MemoryStats> {
        let store = self.store.read().await;
        let memories = store.memories.values();
        let total_memories = memories.len();
        let mut total_importance = 0.0;
        let mut total_size = 0;
        let mut context_distribution = HashMap::new();
        let mut relationship_counts = HashMap::new();

        for memory in memories {
            total_importance += memory.attributes.importance;
            total_size += memory.vector.data.len();
            
            *context_distribution
                .entry(memory.attributes.context.clone())
                .or_insert(0) += 1;

            for rel in &memory.attributes.relationships {
                *relationship_counts.entry(rel.clone()).or_insert(0) += 1;
            }
        }

        let avg_vector_size = if total_memories > 0 {
            total_size as f64 / total_memories as f64
        } else {
            0.0
        };

        let average_importance = if total_memories > 0 {
            total_importance / total_memories as f32
        } else {
            0.0
        };

        let capacity_used = total_size as f64;

        // Get most connected memories
        let mut most_connected: Vec<_> = relationship_counts.into_iter().collect();
        most_connected.sort_by(|a, b| b.1.cmp(&a.1));
        let most_connected_memories = most_connected
            .into_iter()
            .take(10)
            .map(|(id, _)| id)
            .collect();

        Ok(MemoryStats {
            total_memories,
            total_size,
            avg_vector_size,
            capacity_used,
            average_importance,
            context_distribution,
            most_connected_memories,
        })
    }

    #[tracing::instrument(skip(self))]
    async fn backup(&self, path: PathBuf) -> Result<()> {
        self.save_to_file(&path).await
    }

    #[tracing::instrument(skip(self))]
    async fn restore(&mut self, path: PathBuf) -> Result<()> {
        self.load_from_file(&path).await
    }
}
