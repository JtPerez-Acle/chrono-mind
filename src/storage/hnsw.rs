use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, Duration},
};
use tokio::sync::RwLock;
use rand::random;

use crate::{
    core::error::{MemoryError, Result},
    memory::types::TemporalVector,
    storage::metrics::DistanceMetric,
};

#[derive(Debug, Clone)]
pub struct HNSWConfig {
    pub max_dimensions: usize,
    pub max_connections: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub temporal_weight: f32,
}

impl Default for HNSWConfig {
    fn default() -> Self {
        Self {
            max_dimensions: 3,
            max_connections: 16,
            ef_construction: 10,
            ef_search: 10,
            temporal_weight: 0.1,
        }
    }
}

#[derive(Debug)]
struct Node {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    layer: usize,
    connections: Vec<Vec<String>>,
    vector: Vec<f32>,
    #[allow(dead_code)]
    temporal_score: f32,
    #[allow(dead_code)]
    timestamp: SystemTime,
}

impl Node {
    fn new(id: String, vector: Vec<f32>, layer: usize, temporal_score: f32) -> Self {
        Self {
            id,
            layer,
            connections: vec![Vec::new(); layer + 1],
            vector,
            temporal_score,
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    id: String,
    distance: f32,
    #[allow(dead_code)]
    temporal_score: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Combine distance and temporal_score for comparison
        // Lower distance and higher temporal_score is better
        let self_score = self.distance / self.temporal_score;
        let other_score = other.distance / other.temporal_score;
        other_score.partial_cmp(&self_score)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

pub struct TemporalHNSW {
    config: HNSWConfig,
    nodes: RwLock<HashMap<String, Node>>,
    entry_points: RwLock<Vec<String>>,
    distance_metric: Arc<dyn DistanceMetric + Send + Sync>,
}

impl TemporalHNSW {
    pub fn new(
        config: HNSWConfig,
        distance_metric: Arc<dyn DistanceMetric + Send + Sync>,
    ) -> Self {
        Self {
            config,
            nodes: RwLock::new(HashMap::new()),
            entry_points: RwLock::new(Vec::new()),
            distance_metric,
        }
    }

    /// Normalize a vector to unit length
    fn normalize_vector(&self, v: &[f32]) -> Vec<f32> {
        let magnitude = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            v.iter().map(|x| x / magnitude).collect()
        } else {
            v.to_vec()
        }
    }

    pub async fn insert(&self, temporal: &TemporalVector) -> Result<()> {
        self.validate_dimensions(&temporal.vector.data)?;

        // Normalize vector before insertion
        let normalized_vector = self.normalize_vector(&temporal.vector.data);

        let mut nodes = self.nodes.write().await;
        let mut entry_points = self.entry_points.write().await;
        let max_layer = self.get_random_layer();

        // Calculate temporal score for the new node
        let now = SystemTime::now();
        let age = now.duration_since(temporal.attributes.timestamp)
            .unwrap_or(Duration::from_secs(0))
            .as_secs_f32();
        let temporal_score = (-0.1 * age).exp();

        let mut new_node = Node::new(
            temporal.vector.id.clone(),
            normalized_vector.clone(),  
            max_layer,
            temporal_score,
        );
        new_node.timestamp = temporal.attributes.timestamp;

        let mut curr_ep = if let Some(ep) = entry_points.last() {
            Some(ep.clone())
        } else {
            None
        };

        // Insert edges from max_layer down to 0
        for layer in (0..=max_layer).rev() {
            let candidates = if let Some(ref ep) = curr_ep {
                self.search_layer(
                    &*nodes,
                    &normalized_vector,  
                    Some(ep),
                    1,
                    layer,
                ).await?
            } else {
                Vec::new()
            };

            // Update entry point for next layer
            if !candidates.is_empty() {
                curr_ep = Some(candidates[0].id.clone());
            }

            // Select neighbors for the current layer
            let mut neighbors = Vec::new();
            for candidate in candidates.iter() {
                if neighbors.len() >= self.config.max_connections {
                    break;
                }
                neighbors.push(candidate.id.clone());
            }

            // Add connections
            while new_node.connections.len() <= layer {
                new_node.connections.push(Vec::new());
            }
            new_node.connections[layer] = neighbors.clone();

            // Add reverse connections
            for neighbor_id in neighbors {
                if let Some(neighbor) = nodes.get_mut(&neighbor_id) {
                    while neighbor.connections.len() <= layer {
                        neighbor.connections.push(Vec::new());
                    }
                    if !neighbor.connections[layer].contains(&temporal.vector.id) {
                        neighbor.connections[layer].push(temporal.vector.id.clone());
                    }
                }
            }
        }

        // Update entry points if needed
        if entry_points.len() <= max_layer {
            entry_points.resize(max_layer + 1, temporal.vector.id.clone());
        }

        // Insert the new node
        nodes.insert(temporal.vector.id.clone(), new_node);

        Ok(())
    }

    pub async fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        self.validate_dimensions(query)?;

        // Normalize query vector before search
        let normalized_query = self.normalize_vector(query);

        let nodes = self.nodes.read().await;
        let entry_points = self.entry_points.read().await;

        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let candidates = if let Some(ep) = entry_points.last() {
            self.search_layer(
                &*nodes,
                &normalized_query,  
                Some(ep),
                self.config.ef_search,
                0,
            ).await?
        } else {
            Vec::new()
        };

        // Convert candidates to final results
        let mut scored_candidates: Vec<_> = candidates.into_iter()
            .map(|c| {
                let temporal_weight = self.config.temporal_weight;
                let score = (1.0 - temporal_weight) * c.distance + 
                           temporal_weight * (1.0 - c.temporal_score);
                (c.id, score, c.temporal_score)
            })
            .collect();

        // Sort by combined score
        scored_candidates.sort_by(|a, b| {
            let (_, a_score, _) = a;
            let (_, b_score, _) = b;
            a_score.partial_cmp(b_score).unwrap()
        });
        
        // Truncate and convert back to original format
        scored_candidates.truncate(k);
        Ok(scored_candidates.into_iter()
            .map(|(id, score, _)| (id, score))
            .collect())
    }

    #[allow(dead_code)]
    fn temporal_score(&self, id: &str, now: SystemTime, nodes: &HashMap<String, Node>) -> f32 {
        if let Some(node) = nodes.get(id) {
            let age = now.duration_since(node.timestamp)
                .unwrap_or(Duration::from_secs(0))
                .as_secs_f32();
            (-0.1 * age).exp()  // Exponential decay: 1.0 for now, decaying over time
        } else {
            0.0
        }
    }

    async fn search_layer(
        &self,
        nodes: &HashMap<String, Node>,
        query: &[f32],
        entry_point: Option<&String>,
        ef: usize,
        layer: usize,
    ) -> Result<Vec<Candidate>> {
        use std::collections::BinaryHeap;

        let mut visited = HashMap::new();
        let mut candidates = BinaryHeap::new();
        let mut best_candidates = BinaryHeap::new();

        if let Some(ep) = entry_point {
            if let Some(node) = nodes.get(ep) {
                if layer < node.connections.len() {
                    let dist = self.distance_metric.calculate_distance(&node.vector, query);
                    visited.insert(ep.clone(), dist);
                    let candidate = Candidate {
                        id: ep.clone(),
                        distance: dist,
                        temporal_score: node.temporal_score,
                    };
                    candidates.push(candidate.clone());
                    best_candidates.push(candidate);
                }
            }
        }

        while let Some(current) = candidates.pop() {
            let worst_candidate = best_candidates.peek();
            if let Some(worst) = worst_candidate {
                // Use weighted score for comparison
                let temporal_weight = self.config.temporal_weight;
                let current_score = (1.0 - temporal_weight) * current.distance + 
                                  temporal_weight * (1.0 - current.temporal_score);
                let worst_score = (1.0 - temporal_weight) * worst.distance + 
                                temporal_weight * (1.0 - worst.temporal_score);
                if current_score > worst_score {
                    break;
                }
            }

            if let Some(node) = nodes.get(&current.id) {
                if layer < node.connections.len() {
                    for neighbor_id in &node.connections[layer] {
                        if !visited.contains_key(neighbor_id) {
                            if let Some(neighbor) = nodes.get(neighbor_id) {
                                let dist = self.distance_metric.calculate_distance(&neighbor.vector, query);
                                visited.insert(neighbor_id.clone(), dist);
                                
                                let candidate = Candidate {
                                    id: neighbor_id.clone(),
                                    distance: dist,
                                    temporal_score: neighbor.temporal_score,
                                };
                                
                                // Use weighted score for comparison
                                let temporal_weight = self.config.temporal_weight;
                                let candidate_score = (1.0 - temporal_weight) * dist + 
                                                    temporal_weight * (1.0 - candidate.temporal_score);
                                let should_add = if best_candidates.len() < ef {
                                    true
                                } else {
                                    let worst = best_candidates.peek().unwrap();
                                    let worst_score = (1.0 - temporal_weight) * worst.distance + 
                                                    temporal_weight * (1.0 - worst.temporal_score);
                                    candidate_score < worst_score
                                };
                                
                                if should_add {
                                    candidates.push(candidate.clone());
                                    best_candidates.push(candidate);
                                    if best_candidates.len() > ef {
                                        best_candidates.pop();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut result: Vec<_> = best_candidates.into_iter().collect();
        result.sort_by(|a, b| {
            // Use weighted score for final sorting
            let temporal_weight = self.config.temporal_weight;
            let a_score = (1.0 - temporal_weight) * a.distance + 
                         temporal_weight * (1.0 - a.temporal_score);
            let b_score = (1.0 - temporal_weight) * b.distance + 
                         temporal_weight * (1.0 - b.temporal_score);
            a_score.partial_cmp(&b_score).unwrap()
        });
        Ok(result)
    }

    fn get_random_layer(&self) -> usize {
        let mut layer = 0;
        while random::<f32>() < 0.5 && layer < self.config.max_dimensions {
            layer += 1;
        }
        layer
    }

    fn validate_dimensions(&self, vector: &[f32]) -> Result<()> {
        if vector.len() != self.config.max_dimensions {
            return Err(MemoryError::InvalidDimensions {
                got: vector.len(),
                expected: self.config.max_dimensions,
            });
        }
        Ok(())
    }

    pub async fn get_layer_stats(&self) -> Result<LayerStats> {
        let nodes = self.nodes.read().await;
        let mut layer_sizes = HashMap::new();
        let mut total_connections = 0;

        for (_, node) in nodes.iter() {
            let layer = node.layer;
            *layer_sizes.entry(layer).or_insert(0) += 1;
            total_connections += node.connections.iter().map(|c| c.len()).sum::<usize>();
        }

        let num_layers = if layer_sizes.is_empty() { 0 } else {
            layer_sizes.keys().max().unwrap_or(&0) + 1
        };

        let total_nodes = nodes.len();
        let avg_connections = if total_nodes > 0 {
            total_connections as f64 / total_nodes as f64
        } else {
            0.0
        };

        Ok(LayerStats {
            num_layers,
            total_nodes,
            total_connections,
            avg_connections,
            layer_sizes,
        })
    }
}

#[derive(Debug)]
pub struct LayerStats {
    pub num_layers: usize,
    pub total_nodes: usize,
    pub total_connections: usize,
    pub avg_connections: f64,
    pub layer_sizes: HashMap<usize, usize>,
}
