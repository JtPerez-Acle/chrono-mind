use std::collections::{BinaryHeap, HashSet};
use std::cmp::Ordering;
use crate::error::Result;
use crate::storage::hnsw::node::Node;
use crate::storage::metrics::DistanceMetric;
use crate::storage::hnsw::Config;

#[derive(Debug, Clone, Eq)]
struct Candidate {
    id: String,
    distance: f32,
}

impl Candidate {
    fn new(id: String, distance: f32) -> Self {
        Self { id, distance }
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance.eq(&other.distance)
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

pub fn search(
    query: &[f32],
    k: usize,
    entry_point: &str,
    nodes: &std::collections::HashMap<String, Node>,
    config: &Config,
    metric: &dyn DistanceMetric,
) -> Result<Vec<(String, f32)>> {
    let mut visited = HashSet::new();
    let mut candidates = BinaryHeap::new();
    let mut results = BinaryHeap::new();

    // Get the entry point node
    let entry_node = nodes.get(entry_point).ok_or_else(|| {
        crate::error::VectorStoreError::NotFound(entry_point.to_string())
    })?;
    
    let entry_dist = metric.distance(query, entry_node.vector());
    candidates.push(Candidate::new(entry_point.to_string(), entry_dist));
    visited.insert(entry_point.to_string());

    // Start from the highest layer
    let mut current_layer = entry_node.max_layer;

    while current_layer >= 0 {
        let mut layer_candidates = BinaryHeap::new();
        let ef = if current_layer == 0 { k } else { config.ef };

        while let Some(current) = candidates.pop() {
            let current_node = nodes.get(&current.id).ok_or_else(|| {
                crate::error::VectorStoreError::NotFound(current.id.clone())
            })?;

            // Update results for the bottom layer
            if current_layer == 0 {
                results.push(Candidate::new(current.id.clone(), current.distance));
                if results.len() > k {
                    results.pop();
                }
            }

            // Check neighbors at the current layer
            if let Some(neighbors) = current_node.get_connections(current_layer) {
                for neighbor_id in neighbors {
                    if !visited.contains(neighbor_id) {
                        visited.insert(neighbor_id.to_string());
                        let neighbor_node = nodes.get(neighbor_id).ok_or_else(|| {
                            crate::error::VectorStoreError::NotFound(neighbor_id.clone())
                        })?;
                        
                        let dist = metric.distance(query, neighbor_node.vector());
                        
                        if layer_candidates.is_empty() || layer_candidates.len() < ef || dist < layer_candidates.peek().map(|c| c.distance).unwrap_or(f32::MAX) {
                            layer_candidates.push(Candidate::new(neighbor_id.to_string(), dist));
                            if layer_candidates.len() > ef {
                                layer_candidates.pop();
                            }
                        }
                    }
                }
            }
        }

        candidates = layer_candidates;
        if current_layer == 0 {
            break;
        }
        current_layer -= 1;
    }

    Ok(results.into_sorted_vec().into_iter().map(|c| (c.id, c.distance)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::metrics::EuclideanDistance;

    #[test]
    fn test_search_empty_graph() {
        let nodes = std::collections::HashMap::new();
        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let metric = EuclideanDistance;
        
        let result = search(
            &[1.0, 0.0],
            5,
            "non_existent",
            &nodes,
            &config,
            &metric,
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_search_single_node() {
        let mut nodes = std::collections::HashMap::new();
        let node = Node::new(
            "test".to_string(),
            vec![1.0, 0.0],
            4,
            0,
        );
        nodes.insert("test".to_string(), node);

        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let metric = EuclideanDistance;
        
        let result = search(
            &[1.0, 0.0],
            1,
            "test",
            &nodes,
            &config,
            &metric,
        ).unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "test");
        assert_eq!(result[0].1, 0.0);
    }
}
