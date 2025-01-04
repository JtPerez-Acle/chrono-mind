use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as TokioRwLock;
use tracing::{debug, info, warn};

use super::config::MemoryConfig;
use super::error::MemoryError;
use super::metrics::DistanceMetric;
use super::Vector;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAttributes {
    pub timestamp: SystemTime,
    pub importance: f32,
    pub context: String,
    pub decay_rate: f32,
    pub relationships: Vec<String>,
    pub access_count: u32,
    pub last_access: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalVector {
    pub vector: Vector,
    pub attributes: MemoryAttributes,
}

#[derive(Debug)]
pub struct ContextSummary {
    pub context: String,
    pub importance: f32,
    pub memory_count: usize,
    pub average_vector: Vec<f32>,
    pub key_relationships: Vec<String>,
}

#[derive(Debug)]
pub struct MemoryStorage {
    memories: Arc<TokioRwLock<HashMap<String, TemporalVector>>>,
    metric: Arc<Box<dyn DistanceMetric + Send + Sync>>,
    config: Arc<RwLock<MemoryConfig>>,
    last_cleanup: Arc<RwLock<SystemTime>>,
}

impl MemoryStorage {
    pub fn new(metric: impl DistanceMetric + Send + Sync + 'static, config: MemoryConfig) -> Result<Self, MemoryError> {
        config.validate().map_err(|e| MemoryError::InvalidAttributes(e))?;
        
        info!("Initializing temporal memory storage with config: {:?}", config);
        Ok(Self {
            memories: Arc::new(TokioRwLock::new(HashMap::new())),
            metric: Arc::new(Box::new(metric)),
            config: Arc::new(RwLock::new(config)),
            last_cleanup: Arc::new(RwLock::new(SystemTime::now())),
        })
    }

    pub async fn insert_memory(&self, memory: TemporalVector) -> Result<(), MemoryError> {
        // Validate vector dimensions
        let config = self.config.read().map_err(|_| 
            MemoryError::ConcurrencyError("Failed to read config".to_string()))?;
            
        if memory.vector.data.len() > config.max_dimensions {
            return Err(MemoryError::DimensionMismatch {
                expected: config.max_dimensions,
                actual: memory.vector.data.len(),
            });
        }

        let mut memories = self.memories.write().await;
        if memories.len() >= config.max_memories {
            // Try cleanup before rejecting
            drop(memories); // Release lock before cleanup
            self.cleanup_old_memories().await?;
            
            memories = self.memories.write().await;
            if memories.len() >= config.max_memories {
                return Err(MemoryError::CapacityExceeded(
                    format!("Maximum capacity of {} memories reached", config.max_memories)
                ));
            }
        }

        memories.insert(memory.vector.id.clone(), memory);
        Ok(())
    }

    async fn cleanup_old_memories(&self) -> Result<(), MemoryError> {
        let config = self.config.read().map_err(|_| 
            MemoryError::ConcurrencyError("Failed to read config".to_string()))?;
            
        if !config.enable_auto_cleanup {
            return Ok(());
        }

        let mut last_cleanup = self.last_cleanup.write().map_err(|_|
            MemoryError::ConcurrencyError("Failed to access cleanup timestamp".to_string()))?;
            
        let now = SystemTime::now();
        if now.duration_since(*last_cleanup)? < config.cleanup_interval {
            return Ok(());
        }

        let mut memories = self.memories.write().await;
        let mut to_remove = Vec::new();

        for (id, memory) in memories.iter() {
            if memory.attributes.importance < config.min_importance_threshold {
                to_remove.push(id.clone());
            }
        }

        for id in to_remove {
            memories.remove(&id);
        }

        *last_cleanup = now;
        Ok(())
    }

    pub async fn get_memory(&self, id: &str) -> Result<Option<TemporalVector>, MemoryError> {
        let memories = self.memories.read().await;
        if let Some(memory) = memories.get(id) {
            // Update access metadata
            let mut memory = self.memories.write().await;
            if let Some(memory) = memory.get_mut(id) {
                memory.attributes.access_count += 1;
                memory.attributes.last_access = SystemTime::now();
            }
            Ok(Some(memory.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn get_important_memories(&self, threshold: f32) -> Result<Vec<TemporalVector>, MemoryError> {
        let memories = self.memories.read().await;
        Ok(memories
            .values()
            .filter(|m| m.attributes.importance >= threshold)
            .cloned()
            .collect())
    }

    pub async fn search_by_context(&self, context: &str, limit: usize) -> Result<Vec<TemporalVector>, MemoryError> {
        let memories = self.memories.read().await;
        Ok(memories
            .values()
            .filter(|m| m.attributes.context == context)
            .take(limit)
            .cloned()
            .collect())
    }

    pub async fn get_related_memories(&self, id: &str) -> Result<Vec<TemporalVector>, MemoryError> {
        let memories = self.memories.read().await;
        if let Some(memory) = memories.get(id) {
            Ok(memory
                .attributes
                .relationships
                .iter()
                .filter_map(|rel_id| memories.get(rel_id))
                .cloned()
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn apply_decay(&self, duration: Duration) -> Result<(), MemoryError> {
        let now = SystemTime::now();
        let mut memories = self.memories.write().await;
        for memory in memories.values_mut() {
            if let Ok(elapsed) = now.duration_since(memory.attributes.timestamp) {
                let decay_factor = (elapsed.as_secs_f32() / duration.as_secs_f32())
                    * memory.attributes.decay_rate;
                memory.attributes.importance *= (1.0 - decay_factor).max(0.0);
            }
        }
        Ok(())
    }

    pub async fn consolidate_memories(&self, time_window: Duration) -> Result<(), MemoryError> {
        let now = SystemTime::now();
        let mut consolidation_candidates = Vec::new();

        // Find memories within the time window
        let memories = self.memories.read().await;
        for memory in memories.values() {
            if let Ok(elapsed) = now.duration_since(memory.attributes.timestamp) {
                if elapsed <= time_window {
                    consolidation_candidates.push(memory.clone());
                }
            }
        }

        // Group by context and consolidate similar memories
        let mut contexts: HashMap<String, Vec<TemporalVector>> = HashMap::new();
        for memory in consolidation_candidates {
            contexts
                .entry(memory.attributes.context.clone())
                .or_default()
                .push(memory);
        }

        for memories in contexts.values() {
            self.consolidate_context_memories(memories).await?;
        }

        Ok(())
    }

    async fn consolidate_context_memories(&self, memories: &[TemporalVector]) -> Result<(), MemoryError> {
        if memories.len() < 2 {
            return Ok(());
        }

        let mut memories = self.memories.write().await;
        for i in 0..memories.len() {
            for j in (i + 1)..memories.len() {
                let similarity = self.metric.similarity(
                    &memories[i].vector.data,
                    &memories[j].vector.data,
                );

                if similarity >= self.config.read().map_err(|_| 
                    MemoryError::ConcurrencyError("Failed to read config".to_string()))?.consolidation_threshold {
                    // Create relationship between similar memories
                    if let Some(memory_i) = memories.get_mut(&memories[i].vector.id) {
                        memory_i.attributes.relationships.push(memories[j].vector.id.clone());
                    }
                    if let Some(memory_j) = memories.get_mut(&memories[j].vector.id) {
                        memory_j.attributes.relationships.push(memories[i].vector.id.clone());
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn compress_memories(&self, context: &str) -> Result<Vec<TemporalVector>, MemoryError> {
        let memories = self.memories.read().await;
        let context_memories: Vec<_> = memories
            .values()
            .filter(|m| m.attributes.context == context)
            .cloned()
            .collect();

        if context_memories.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate average vector for the context
        let dim = context_memories[0].vector.data.len();
        let mut avg_vector = vec![0.0; dim];
        let mut total_importance = 0.0;

        for memory in &context_memories {
            for (avg, &val) in avg_vector.iter_mut().zip(&memory.vector.data) {
                *avg += val * memory.attributes.importance;
            }
            total_importance += memory.attributes.importance;
        }

        for avg in &mut avg_vector {
            *avg /= total_importance;
        }

        // Create compressed memory
        let compressed = TemporalVector {
            vector: Vector {
                id: format!("compressed_{}", context),
                data: avg_vector,
            },
            attributes: MemoryAttributes {
                timestamp: SystemTime::now(),
                importance: total_importance / context_memories.len() as f32,
                context: context.to_string(),
                decay_rate: context_memories.iter().map(|m| m.attributes.decay_rate).sum::<f32>() 
                    / context_memories.len() as f32,
                relationships: context_memories.iter()
                    .flat_map(|m| m.attributes.relationships.clone())
                    .collect(),
                access_count: 0,
                last_access: SystemTime::now(),
            },
        };

        Ok(vec![compressed])
    }

    pub async fn get_context_summary(&self, context: &str) -> Result<Option<ContextSummary>, MemoryError> {
        let memories = self.memories.read().await;
        let context_memories: Vec<_> = memories
            .values()
            .filter(|m| m.attributes.context == context)
            .collect();

        if context_memories.is_empty() {
            return Ok(None);
        }

        let memory_count = context_memories.len();
        let total_importance: f32 = context_memories.iter()
            .map(|m| m.attributes.importance)
            .sum();

        // Calculate average vector
        let dim = context_memories[0].vector.data.len();
        let mut avg_vector = vec![0.0; dim];
        
        for memory in &context_memories {
            for (avg, &val) in avg_vector.iter_mut().zip(&memory.vector.data) {
                *avg += val;
            }
        }

        for avg in &mut avg_vector {
            *avg /= memory_count as f32;
        }

        // Get most common relationships
        let mut relationship_counts: HashMap<String, usize> = HashMap::new();
        for memory in &context_memories {
            for rel in &memory.attributes.relationships {
                *relationship_counts.entry(rel.clone()).or_default() += 1;
            }
        }

        let mut key_relationships: Vec<_> = relationship_counts.into_iter().collect();
        key_relationships.sort_by(|a, b| b.1.cmp(&a.1));
        let key_relationships: Vec<_> = key_relationships.into_iter()
            .take(5)
            .map(|(rel, _)| rel)
            .collect();

        Ok(Some(ContextSummary {
            context: context.to_string(),
            importance: total_importance / memory_count as f32,
            memory_count,
            average_vector: avg_vector,
            key_relationships,
        }))
    }

    pub async fn get_memory_stats(&self) -> Result<MemoryStats, MemoryError> {
        let memories = self.memories.read().await;
        let config = self.config.read().map_err(|_| 
            MemoryError::ConcurrencyError("Failed to read config".to_string()))?;

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

        Ok(MemoryStats {
            total_memories,
            capacity_used: (total_memories as f32 / config.max_memories as f32) * 100.0,
            average_importance: total_importance / total_memories as f32,
            context_distribution: context_counts,
            most_connected_memories: relationship_counts.into_iter()
                .sorted_by(|a, b| b.1.cmp(&a.1))
                .take(10)
                .collect(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct MemoryStats {
    pub total_memories: usize,
    pub capacity_used: f32,
    pub average_importance: f32,
    pub context_distribution: HashMap<String, usize>,
    pub most_connected_memories: Vec<(String, usize)>,
}
