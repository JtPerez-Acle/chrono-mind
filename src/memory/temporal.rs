use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};
use parking_lot::RwLock;
use crate::{
    core::{
        config::MemoryConfig,
        error::{MemoryError, Result},
    },
    storage::metrics::{DistanceMetric, CosineDistance as MetricCosineDistance},
    memory::types::TemporalVector,
};
use hnsw_rs::{
    hnsw::{Hnsw as HnswIndex, Neighbour},
    dist::Distance,
};

#[derive(Clone)]
struct CosineDistance;

impl Distance<Vec<f32>> for CosineDistance {
    fn eval(&self, a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
        if a.is_empty() || b.is_empty() || a[0].len() != b[0].len() {
            return f32::MAX;
        }
        
        let va = &a[0];
        let vb = &b[0];
        
        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;
        
        for i in 0..va.len() {
            dot_product += va[i] * vb[i];
            norm_a += va[i] * va[i];
            norm_b += vb[i] * vb[i];
        }
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return f32::MAX;
        }
        
        let similarity = dot_product / (norm_a.sqrt() * norm_b.sqrt());
        // Convert similarity to distance (0 to MAX)
        if similarity > 1.0 {
            0.0
        } else if similarity < -1.0 {
            2.0
        } else {
            1.0 - similarity
        }
    }
}

pub struct MemoryStorage {
    config: MemoryConfig,
    memories: RwLock<HashMap<String, TemporalVector>>,
    distance_metric: Arc<dyn DistanceMetric + Send + Sync>,
    hnsw: Hnsw, // Add HNSW index
}

impl MemoryStorage {
    pub fn new(
        config: MemoryConfig,
        distance_metric: Arc<dyn DistanceMetric + Send + Sync>,
    ) -> Self {
        Self {
            memories: RwLock::new(HashMap::new()),
            config,
            distance_metric,
            hnsw: Hnsw::new(), // Initialize HNSW index
        }
    }

    pub async fn save_memory(&mut self, memory: TemporalVector) -> Result<()> {
        // Validate dimensions
        if memory.vector.data.len() != self.config.max_dimensions {
            return Err(MemoryError::InvalidDimensions {
                got: memory.vector.data.len(),
                expected: self.config.max_dimensions,
            });
        }

        // Validate importance
        if memory.attributes.importance < 0.0 || memory.attributes.importance > 1.0 {
            return Err(MemoryError::InvalidImportance(memory.attributes.importance));
        }

        // Add to HNSW index first
        self.hnsw.add(&memory.vector.data, memory.vector.id.clone())
            .map_err(|e| MemoryError::HnswError(e.to_string()))?;

        // Then add to memory store
        {
            let mut memories = self.memories.write();

            // If memory already exists, merge relationships
            if let Some(existing) = memories.get(&memory.vector.id) {
                let mut updated = memory.clone();
                let mut relationships: HashSet<_> = existing.attributes.relationships.iter().cloned().collect();
                relationships.extend(updated.attributes.relationships.iter().cloned());
                updated.attributes.relationships = relationships.into_iter().collect();
                memories.insert(memory.vector.id.clone(), updated);
            } else {
                memories.insert(memory.vector.id.clone(), memory);
            }
        }

        Ok(())
    }

    pub async fn get_memory(&self, id: &str) -> Result<Option<TemporalVector>> {
        let memories = self.memories.read();
        Ok(memories.get(id).cloned())
    }

    pub async fn search_similar(&self, query: &[f32], k: usize) -> Result<Vec<(TemporalVector, f32)>> {
        let memories = self.memories.read();
        let now = SystemTime::now();

        // Use HNSW for approximate nearest neighbor search
        let candidates = self.hnsw.search(query, k * 2)?; // Get more candidates to account for temporal reranking
        
        // Convert results and apply temporal scoring
        let mut results: Vec<_> = candidates.into_iter()
            .filter_map(|(id, distance)| {
                memories.get(&id).map(|m| {
                    let time_diff = now.duration_since(m.attributes.timestamp)
                        .unwrap_or(Duration::from_secs(0))
                        .as_secs_f32();
                    
                    // Temporal score: more recent = higher score (closer to 1)
                    let temporal_score = (-self.config.base_decay_rate * time_diff).exp();
                    
                    // Distance is already normalized to [0, 2], temporal_score is [0, 1]
                    // Scale temporal score to [0, 2] to match distance range
                    let temporal_distance = 2.0 * (1.0 - temporal_score);
                    
                    // Combine distance and temporal scores with proper weighting
                    let combined_score = 
                        distance * (1.0 - self.config.temporal_weight) + 
                        temporal_distance * self.config.temporal_weight;
                    
                    // Adjust score based on importance (higher importance = lower score)
                    let final_score = combined_score / (1.0 + m.attributes.importance);
                    
                    (m.clone(), final_score)
                })
            })
            .collect();
        
        // Sort by final score (lower is better)
        results.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        
        Ok(results)
    }

    pub async fn update_memory_decay(&mut self) -> Result<()> {
        let now = SystemTime::now();
        let mut memories = self.memories.write();

        for memory in memories.values_mut() {
            let age = now.duration_since(memory.attributes.timestamp)
                .unwrap_or(Duration::from_secs(0))
                .as_secs() as f32 / 3600.0; // Convert to hours

            let recency = now.duration_since(memory.attributes.last_access)
                .unwrap_or(Duration::from_secs(0))
                .as_secs() as f32 / 3600.0;

            let access_factor = 1.0 / (1.0 + memory.attributes.access_count as f32).ln();
            let decay = self.config.base_decay_rate * age * access_factor * recency;
            
            memory.attributes.importance = (memory.attributes.importance * (1.0 - decay))
                .max(self.config.min_importance)
                .min(self.config.max_importance);
        }

        Ok(())
    }

    pub async fn get_context_summary(&self, context: &str) -> Result<ContextSummary> {
        let memories = self.memories.read();
        let context_memories: Vec<_> = memories.values()
            .filter(|m| m.attributes.context == context)
            .collect();

        if context_memories.is_empty() {
            return Ok(ContextSummary {
                memory_count: 0,
                average_importance: 0.0,
            });
        }

        Ok(ContextSummary {
            memory_count: context_memories.len(),
            average_importance: context_memories.iter()
                .map(|m| m.attributes.importance)
                .sum::<f32>() / context_memories.len() as f32,
        })
    }

    pub async fn search_by_context(&self, context: &str, query: &[f32], k: usize) -> Result<Vec<(TemporalVector, f32)>> {
        let memories = self.memories.read();
        let now = SystemTime::now();
        
        let context_memories: Vec<_> = memories.values()
            .filter(|m| m.attributes.context == context)
            .collect();
        
        let mut results: Vec<_> = context_memories.into_iter()
            .map(|m| {
                let distance = self.distance_metric.calculate_distance(&m.vector.data, query);
                let time_diff = now.duration_since(m.attributes.timestamp)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs_f32();
                
                // Temporal score: more recent = higher score (closer to 1)
                let temporal_score = (-self.config.base_decay_rate * time_diff).exp();
                
                // Combine scores with configurable weights
                let similarity_weight = 0.4;
                let temporal_weight = 0.4;
                let importance_weight = 0.2;
                
                let score = (distance * similarity_weight) -
                           (temporal_score * temporal_weight) -
                           (m.attributes.importance * importance_weight);
                
                (m.clone(), score)
            })
            .collect();
        
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        
        Ok(results)
    }

    pub async fn get_related_memories(&self, id: &str, max_depth: usize) -> Result<Vec<TemporalVector>> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        if let Some(memory) = self.get_memory(id).await? {
            queue.push_back((memory, 0));
            visited.insert(id.to_string());
        }

        while let Some((memory, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            for rel_id in &memory.attributes.relationships {
                if !visited.contains(rel_id) {
                    if let Some(rel_memory) = self.get_memory(rel_id).await? {
                        visited.insert(rel_id.clone());
                        result.push(rel_memory.clone());
                        queue.push_back((rel_memory, depth + 1));
                    }
                }
            }
        }

        Ok(result)
    }

    pub async fn consolidate_memories(&mut self) -> Result<()> {
        let memories = self.memories.read();
        let mut to_consolidate = Vec::new();

        for (id1, m1) in memories.iter() {
            for (id2, m2) in memories.iter() {
                if id1 >= id2 {
                    continue;
                }

                let similarity = 1.0 - self.distance_metric.calculate_distance(&m1.vector.data, &m2.vector.data);
                if similarity > self.config.similarity_threshold {
                    to_consolidate.push((id1.clone(), id2.clone()));
                }
            }
        }
        drop(memories);

        for (id1, id2) in to_consolidate {
            let mut memories = self.memories.write();
            if let (Some(m1), Some(m2)) = (memories.get(&id1), memories.get(&id2)) {
                let new_importance = (m1.attributes.importance + m2.attributes.importance) / 2.0;
                let mut consolidated = m1.clone();
                consolidated.attributes.importance = new_importance;
                memories.insert(id1, consolidated);
                memories.remove(&id2);
            }
        }

        Ok(())
    }

    pub async fn list_memories(&self) -> Result<Vec<TemporalVector>> {
        let memories = self.memories.read();
        Ok(memories.values().cloned().collect())
    }

    pub async fn get_memory_count(&self) -> usize {
        self.memories.read().len()
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        let config = MemoryConfig::default();
        let distance_metric = Arc::new(MetricCosineDistance::new());
        Self {
            config,
            memories: RwLock::new(HashMap::new()),
            distance_metric,
            hnsw: Hnsw::new(),
        }
    }
}

#[derive(Debug)]
pub struct ContextSummary {
    pub memory_count: usize,
    pub average_importance: f32,
}

struct Hnsw {
    index: HnswIndex<Vec<f32>, CosineDistance>,
    next_id: usize,
    id_map: HashMap<String, usize>,
}

impl Hnsw {
    fn new() -> Self {
        let max_nb_connection = 16;
        let max_elements = 10_000;
        let max_layer = 16;
        let ef_construction = 200;
        
        Hnsw {
            index: HnswIndex::new(
                max_nb_connection,
                max_elements,
                max_layer,
                ef_construction,
                CosineDistance,
            ),
            next_id: 0,
            id_map: HashMap::new(),
        }
    }

    fn add(&mut self, data: &[f32], id: String) -> Result<()> {
        let node_id = self.next_id;
        self.next_id += 1;
        
        self.index.insert((&vec![data.to_vec()], node_id));
        self.id_map.insert(id, node_id);
        Ok(())
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let ef = k * 2; // Use larger ef for better recall
        let neighbors = self.index.search(&[query.to_vec()], k, ef);
        
        Ok(neighbors.into_iter()
            .filter_map(|n| {
                self.id_map.iter()
                    .find(|(_, &id)| id == n.d_id)
                    .map(|(key, _)| (key.clone(), n.distance))
            })
            .collect())
    }
}
