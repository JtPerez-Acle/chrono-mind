use std::{
    collections::{BinaryHeap, HashMap},
    sync::Arc,
};
use tokio::sync::RwLock;

use crate::{
    core::error::{MemoryError, Result},
    memory::types::{TemporalVector, Vector},
    storage::metrics::DistanceMetric,
};

/// Node in the HNSW graph
#[derive(Clone, Debug)]
struct Node {
    vector: Vector,
    connections: Vec<Vec<String>>, // Connections at each layer
    _temporal_score: f32,           // Combined score of importance and recency
}

/// Candidate for neighbor search
#[derive(PartialEq)]
struct Candidate {
    id: String,
    distance: f32,
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Configuration for HNSW graph
#[derive(Clone, Debug)]
pub struct HNSWConfig {
    pub max_layers: usize,         // Maximum number of layers
    pub ef_construction: usize,    // Size of dynamic candidate list for construction
    pub max_connections: usize,    // Maximum connections per node
    pub temporal_weight: f32,      // Weight for temporal score (0-1)
}

impl Default for HNSWConfig {
    fn default() -> Self {
        Self {
            max_layers: 16,
            ef_construction: 100,
            max_connections: 32,
            temporal_weight: 0.3,
        }
    }
}

/// Temporal-aware HNSW implementation
pub struct TemporalHNSW {
    nodes: Arc<RwLock<HashMap<String, Node>>>,
    entry_points: Arc<RwLock<Vec<String>>>,
    metric: Arc<dyn DistanceMetric>,
    config: HNSWConfig,
}

impl TemporalHNSW {
    /// Create a new HNSW index
    pub fn new(metric: impl DistanceMetric + 'static, config: HNSWConfig) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            entry_points: Arc::new(RwLock::new(Vec::new())),
            metric: Arc::new(metric),
            config,
        }
    }

    /// Insert a temporal vector into the index
    pub async fn insert(&self, temporal: &TemporalVector) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let mut entry_points = self.entry_points.write().await;

        // Calculate temporal score
        let temporal_score = self.calculate_temporal_score(temporal);

        // Create node
        let node = Node {
            vector: temporal.vector.clone(),
            connections: vec![Vec::new(); self.config.max_layers],
            _temporal_score: temporal_score,
        };

        // Insert node
        if nodes.is_empty() {
            // First node becomes entry point
            nodes.insert(temporal.vector.id.clone(), node);
            entry_points.push(temporal.vector.id.clone());
            return Ok(());
        }

        // Insert node first
        nodes.insert(temporal.vector.id.clone(), node);

        // Find insertion layer
        let max_layer = self.get_random_layer();
        let mut current_layer = entry_points.len() - 1;

        // Start from top layer
        let mut ep = entry_points[current_layer].clone();
        let mut ep_dist = self.calculate_distance(&nodes[&ep].vector, &temporal.vector)?;

        // Search through layers
        while current_layer > max_layer {
            let next_ep = self.search_layer(
                &nodes,
                &ep,
                &temporal.vector,
                1,
                current_layer,
            ).await?;

            if let Some((next_id, next_dist)) = next_ep.first().map(|c| (c.id.clone(), c.distance)) {
                if next_dist < ep_dist {
                    ep = next_id;
                    ep_dist = next_dist;
                }
            }

            current_layer -= 1;
        }

        // Insert connections at each layer
        for layer in 0..=max_layer {
            let neighbors = self.search_layer(
                &nodes,
                &ep,
                &temporal.vector,
                self.config.ef_construction,
                layer,
            ).await?;

            let selected = self.select_neighbors(
                &nodes,
                &neighbors,
                self.config.max_connections,
                temporal_score,
            );

            // Add bidirectional connections
            let node_id = temporal.vector.id.clone();
            for neighbor in selected {
                nodes.get_mut(&neighbor)
                    .ok_or_else(|| MemoryError::NotFound(neighbor.clone()))?
                    .connections[layer].push(node_id.clone());
                nodes.get_mut(&node_id)
                    .ok_or_else(|| MemoryError::NotFound(node_id.clone()))?
                    .connections[layer].push(neighbor);
            }
        }

        // Update entry point if necessary
        if max_layer > entry_points.len() - 1 {
            entry_points.push(temporal.vector.id.clone());
        }

        tracing::debug!("Inserted vector {} at layer {}", temporal.vector.id, max_layer);
        Ok(())
    }

    /// Search for nearest neighbors
    pub async fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let nodes = self.nodes.read().await;
        let entry_points = self.entry_points.read().await;

        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let mut current_layer = entry_points.len() - 1;
        let mut ep = entry_points[current_layer].clone();
        let query_vector = Vector::new("query".into(), query.to_vec());
        let mut ep_dist = self.calculate_distance(&nodes[&ep].vector, &query_vector)?;

        // Search through layers
        while current_layer > 0 {
            let next_ep = self.search_layer(
                &nodes,
                &ep,
                &query_vector,
                1,
                current_layer,
            ).await?;

            if let Some((next_id, next_dist)) = next_ep.first().map(|c| (c.id.clone(), c.distance)) {
                if next_dist < ep_dist {
                    ep = next_id;
                    ep_dist = next_dist;
                }
            }

            current_layer -= 1;
        }

        // Get final candidates
        let mut candidates = self.search_layer(
            &nodes,
            &ep,
            &query_vector,
            k,
            0,
        ).await?;

        // Calculate max distance for normalization
        let max_dist = candidates.iter()
            .map(|c| c.distance)
            .fold(0./0., f32::max);

        // Sort by combined score (distance and temporal)
        candidates.sort_by(|a, b| {
            if self.config.temporal_weight > 0.0 {
                let a_time_score = nodes[&a.id]._temporal_score;
                let b_time_score = nodes[&b.id]._temporal_score;
                
                // Normalize distance scores to [0, 1] range
                let a_dist_score = a.distance / max_dist;
                let b_dist_score = b.distance / max_dist;
                
                // Combine scores with temporal weight
                let a_score = (1.0 - a_dist_score) * (1.0 - self.config.temporal_weight)
                    + a_time_score * self.config.temporal_weight;
                let b_score = (1.0 - b_dist_score) * (1.0 - self.config.temporal_weight)
                    + b_time_score * self.config.temporal_weight;
                    
                // Sort in descending order (higher scores are better)
                b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                // Pure distance-based sorting (lower distances are better)
                a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        Ok(candidates.into_iter()
            .take(k)
            .map(|c| (c.id, c.distance))
            .collect())
    }

    async fn search_layer(
        &self,
        nodes: &HashMap<String, Node>,
        entry_point: &str,
        query: &Vector,
        ef: usize,
        layer: usize,
    ) -> Result<Vec<Candidate>> {
        let mut visited = HashMap::new();
        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();

        // Initialize with entry point
        let dist = self.calculate_distance(&nodes[entry_point].vector, query)?;
        candidates.push(Candidate {
            id: entry_point.to_string(),
            distance: dist,
        });
        results.push(Candidate {
            id: entry_point.to_string(),
            distance: dist,
        });
        visited.insert(entry_point.to_string(), dist);

        while let Some(current) = candidates.pop() {
            if results.peek().map_or(false, |best| current.distance > best.distance) {
                break;
            }

            // Check connections at current layer
            for neighbor_id in &nodes[&current.id].connections[layer] {
                if !visited.contains_key(neighbor_id) {
                    let dist = self.calculate_distance(&nodes[neighbor_id].vector, query)?;
                    visited.insert(neighbor_id.clone(), dist);

                    if results.len() < ef || dist < results.peek().unwrap().distance {
                        candidates.push(Candidate {
                            id: neighbor_id.clone(),
                            distance: dist,
                        });
                        results.push(Candidate {
                            id: neighbor_id.clone(),
                            distance: dist,
                        });

                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        Ok(results.into_sorted_vec())
    }

    fn select_neighbors(
        &self,
        nodes: &HashMap<String, Node>,
        candidates: &[Candidate],
        m: usize,
        temporal_score: f32,
    ) -> Vec<String> {
        let mut selected = Vec::new();
        let mut remaining: Vec<_> = candidates.iter().collect();

        while selected.len() < m && !remaining.is_empty() {
            // Find best candidate considering both distance and temporal score
            let (idx, _) = remaining.iter().enumerate()
                .min_by(|(_, a), (_, b)| {
                    let a_score = a.distance * (1.0 - self.config.temporal_weight)
                        + nodes[&a.id]._temporal_score * self.config.temporal_weight;
                    let b_score = b.distance * (1.0 - self.config.temporal_weight)
                        + nodes[&b.id]._temporal_score * self.config.temporal_weight;
                    a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();

            let best = remaining.remove(idx);
            selected.push(best.id.clone());
        }

        selected
    }

    fn calculate_temporal_score(&self, temporal: &TemporalVector) -> f32 {
        let now = std::time::SystemTime::now();
        let age = now.duration_since(temporal.attributes.timestamp)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs_f32();

        let time_factor = (-temporal.attributes.decay_rate * age).exp();
        time_factor * temporal.attributes.importance
    }

    fn calculate_distance(&self, a: &Vector, b: &Vector) -> Result<f32> {
        if a.data.len() != b.data.len() {
            return Err(MemoryError::DimensionMismatch {
                expected: a.data.len(),
                actual: b.data.len(),
            });
        }
        Ok(1.0 - self.metric.similarity(&a.data, &b.data))
    }

    fn get_random_layer(&self) -> usize {
        let mut layer = 0;
        while layer < self.config.max_layers - 1 && rand::random::<f32>() < 0.5 {
            layer += 1;
        }
        layer
    }
}
