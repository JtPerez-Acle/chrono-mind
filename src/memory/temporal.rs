use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::{
    core::{
        config::MemoryConfig,
        error::{MemoryError, Result},
    },
    memory::types::{ContextSummary, MemoryStats, TemporalVector},
    storage::metrics::DistanceMetric,
    utils::{
        monitoring::PerformanceMonitor,
        validation::{validate_dimensions, validate_temporal_vector},
    },
};

/// Thread-safe memory storage with temporal attributes
#[derive(Clone)]
pub struct MemoryStorage {
    memories: Arc<RwLock<HashMap<String, TemporalVector>>>,
    metric: Arc<dyn DistanceMetric>,
    config: Arc<MemoryConfig>,
}

impl MemoryStorage {
    /// Create a new memory storage instance
    pub fn new(
        metric: impl DistanceMetric + 'static,
        config: MemoryConfig,
    ) -> Result<Self> {
        config.validate().map_err(MemoryError::ConfigError)?;

        Ok(Self {
            memories: Arc::new(RwLock::new(HashMap::new())),
            metric: Arc::new(metric),
            config: Arc::new(config),
        })
    }

    /// Save a memory to storage
    pub async fn save_memory(&mut self, memory: TemporalVector) -> Result<()> {
        let _monitor = PerformanceMonitor::new("save_memory");
        
        // Validate memory
        validate_dimensions(&memory.vector, &self.config)?;
        validate_temporal_vector(&memory)?;

        let mut memories = self.memories.write().await;
        
        // Check capacity
        if memories.len() >= self.config.max_memories && !memories.contains_key(&memory.vector.id) {
            return Err(MemoryError::CapacityExceeded(format!(
                "Maximum memories ({}) reached",
                self.config.max_memories
            )));
        }

        memories.insert(memory.vector.id.clone(), memory);
        debug!("Memory saved successfully");
        Ok(())
    }

    /// Get a memory by ID
    pub async fn get_memory(&self, id: &str) -> Result<Option<TemporalVector>> {
        let _monitor = PerformanceMonitor::new("get_memory");
        let mut memories = self.memories.write().await;
        
        if let Some(memory) = memories.get_mut(id) {
            // Update access stats
            memory.attributes.access_count += 1;
            memory.attributes.last_access = std::time::SystemTime::now();
            Ok(Some(memory.clone()))
        } else {
            Ok(None)
        }
    }

    /// Search for similar memories
    pub async fn search_similar(&self, query: &[f32], limit: usize) -> Result<Vec<(TemporalVector, f32)>> {
        let _monitor = PerformanceMonitor::new("search_similar");
        let memories = self.memories.read().await;
        
        let mut results: Vec<_> = memories
            .values()
            .map(|memory| {
                let similarity = self.metric.similarity(&memory.vector.data, query);
                (memory.clone(), similarity)
            })
            .collect();

        results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results.into_iter().take(limit).collect())
    }

    /// Get memories by context
    pub async fn get_memories_by_context(&self, context: &str) -> Result<Vec<TemporalVector>> {
        let _monitor = PerformanceMonitor::new("get_memories_by_context");
        let memories = self.memories.read().await;
        
        Ok(memories
            .values()
            .filter(|m| m.attributes.context == context)
            .cloned()
            .collect())
    }

    /// Get summary of a context
    pub async fn get_context_summary(&self, context: &str) -> Result<ContextSummary> {
        let _monitor = PerformanceMonitor::new("get_context_summary");
        let memories = self.memories.read().await;
        
        let context_memories: Vec<_> = memories
            .values()
            .filter(|m| m.attributes.context == context)
            .collect();

        if context_memories.is_empty() {
            return Ok(ContextSummary {
                context: context.to_string(),
                importance: 0.0,
                memory_count: 0,
                average_vector: vec![],
                key_relationships: vec![],
            });
        }

        let memory_count = context_memories.len();
        let importance = context_memories.iter().map(|m| m.attributes.importance).sum::<f32>() / memory_count as f32;

        // Calculate average vector
        let dim = context_memories[0].vector.data.len();
        let mut average_vector = vec![0.0; dim];
        for memory in &context_memories {
            for (avg, val) in average_vector.iter_mut().zip(&memory.vector.data) {
                *avg += val;
            }
        }
        for avg in &mut average_vector {
            *avg /= memory_count as f32;
        }

        // Get most common relationships
        let mut relationship_counts = HashMap::new();
        for memory in &context_memories {
            for rel in &memory.attributes.relationships {
                *relationship_counts.entry(rel.clone()).or_insert(0) += 1;
            }
        }

        let mut key_relationships: Vec<_> = relationship_counts.into_iter().collect();
        key_relationships.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        let key_relationships = key_relationships.into_iter()
            .take(5)
            .map(|(rel, _)| rel)
            .collect();

        Ok(ContextSummary {
            context: context.to_string(),
            importance,
            memory_count,
            average_vector,
            key_relationships,
        })
    }

    /// Delete all memories in a context
    pub async fn delete_context(&mut self, context: &str) -> Result<()> {
        let _monitor = PerformanceMonitor::new("delete_context");
        let mut memories = self.memories.write().await;
        
        memories.retain(|_, memory| memory.attributes.context != context);
        Ok(())
    }

    /// Get relationships for a memory
    pub async fn get_relationships(&self, id: &str) -> Result<Vec<TemporalVector>> {
        let _monitor = PerformanceMonitor::new("get_relationships");
        let memories = self.memories.read().await;
        
        let memory = memories.get(id).ok_or_else(|| MemoryError::NotFound(id.to_string()))?;
        
        Ok(memory.attributes.relationships.iter()
            .filter_map(|rel_id| memories.get(rel_id))
            .cloned()
            .collect())
    }

    /// Update memory decay based on time and access patterns
    pub async fn update_memory_decay(&mut self) -> Result<()> {
        let _monitor = PerformanceMonitor::new("update_memory_decay");
        let mut memories = self.memories.write().await;
        
        let now = std::time::SystemTime::now();
        for memory in memories.values_mut() {
            // Calculate time-based decay
            let age = now.duration_since(memory.attributes.timestamp)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_secs_f32();
            
            // Calculate access-based decay
            let time_since_access = now.duration_since(memory.attributes.last_access)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_secs_f32();
            
            // Combine base decay rate with memory's individual rate
            let effective_decay_rate = self.config.base_decay_rate * memory.attributes.decay_rate;
            
            // Calculate decay factors
            let time_factor = (-effective_decay_rate * age).exp();
            let access_factor = (memory.attributes.access_count as f32 + 1.0).ln();
            let recency_factor = (-effective_decay_rate * time_since_access).exp();
            
            // Update importance
            memory.attributes.importance *= time_factor * access_factor * recency_factor;
            memory.attributes.importance = memory.attributes.importance.clamp(
                self.config.min_importance,
                self.config.max_importance,
            );

            debug!(
                "Memory {} decay: time_factor={:.3}, access_factor={:.3}, recency_factor={:.3}, new_importance={:.3}",
                memory.vector.id, time_factor, access_factor, recency_factor, memory.attributes.importance
            );
        }

        // Remove memories below minimum importance
        memories.retain(|_, memory| memory.attributes.importance >= self.config.min_importance);
        Ok(())
    }

    /// Get storage statistics
    pub async fn get_memory_stats(&self) -> Result<MemoryStats> {
        let _monitor = PerformanceMonitor::new("get_memory_stats");
        let memories = self.memories.read().await;
        
        let total_memories = memories.len();
        let mut total_importance = 0.0;
        let mut context_counts = HashMap::new();
        let mut relationship_counts = HashMap::new();

        for memory in memories.values() {
            total_importance += memory.attributes.importance;
            *context_counts.entry(memory.attributes.context.clone()).or_insert(0) += 1;
            
            for rel in &memory.attributes.relationships {
                *relationship_counts.entry(rel.clone()).or_insert(0) += 1;
            }
        }

        let capacity_used = (total_memories as f32 / self.config.max_memories as f32) * 100.0;
        let average_importance = if total_memories > 0 {
            total_importance / total_memories as f32
        } else {
            0.0
        };

        let mut most_connected: Vec<_> = relationship_counts.into_iter().collect();
        most_connected.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        let most_connected = most_connected.into_iter().take(10).collect();

        Ok(MemoryStats {
            total_memories,
            capacity_used,
            average_importance,
            context_distribution: context_counts,
            most_connected_memories: most_connected,
        })
    }

    #[cfg(test)]
    pub async fn get_memory_no_update(&self, id: &str) -> Result<Option<TemporalVector>> {
        let memories = self.memories.read().await;
        Ok(memories.get(id).cloned())
    }
}
