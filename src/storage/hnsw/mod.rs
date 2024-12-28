mod config;
mod node;
mod search;
mod candidate;
mod insert;

pub use config::Config;
pub use node::Node;
pub use search::search;

use std::collections::HashMap;
use async_trait::async_trait;
use crate::error::{Result, VectorStoreError};
use crate::storage::{Vector, VectorStorage};
use crate::storage::metrics::DistanceMetric;
use tracing::{debug, info, warn};

pub struct HnswIndex {
    config: Config,
    nodes: HashMap<String, Node>,
    metric: Box<dyn DistanceMetric>,
    entry_point: Option<String>,
}

impl HnswIndex {
    pub fn new(config: Config, metric: Box<dyn DistanceMetric>) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            metric,
            entry_point: None,
        }
    }

    fn select_level(&self) -> usize {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut level = 0;
        
        while rng.gen::<f32>() < 1.0 / self.config.m as f32 && level < self.config.max_layers - 1 {
            level += 1;
        }
        
        level
    }
}

#[async_trait]
impl VectorStorage for HnswIndex {
    async fn insert(&mut self, vector: Vector) -> Result<()> {
        debug!(
            vector_id = %vector.id,
            dimensions = %vector.data.len(),
            "Inserting vector into HNSW index"
        );

        let layer = if self.nodes.is_empty() {
            info!("Creating first node as entry point");
            self.config.max_layers - 1
        } else {
            debug!("Selecting insertion layer");
            self.select_level()
        };

        let node = Node::new(
            vector.id.clone(),
            vector.data.clone(),
            self.config.max_layers,
            layer,
        );

        // Store the node
        self.nodes.insert(vector.id.clone(), node);

        // If this is the first node, set it as entry point
        if self.entry_point.is_none() {
            info!(
                entry_point = %vector.id,
                "Setting new entry point"
            );
            self.entry_point = Some(vector.id.clone());
        } else {
            // Connect to existing nodes
            let entry_point = self.entry_point.as_ref().unwrap();
            let mut current_layer = layer;
            let mut nearest_neighbor = entry_point.clone();

            while current_layer >= 0 {
                // Find nearest neighbors at current layer
                let neighbors = search(
                    &vector.data,
                    self.config.m,
                    &nearest_neighbor,
                    &self.nodes,
                    &self.config,
                    &*self.metric,
                )?;

                // Connect to neighbors
                let node = self.nodes.get_mut(&vector.id).unwrap();
                for (neighbor_id, _) in neighbors.iter().take(self.config.m) {
                    node.add_connection(current_layer, neighbor_id.clone());
                    
                    // Add reverse connection
                    if let Some(neighbor) = self.nodes.get_mut(neighbor_id) {
                        neighbor.add_connection(current_layer, vector.id.clone());
                    }
                }

                if current_layer == 0 {
                    break;
                }
                current_layer -= 1;
            }
        }

        debug!(
            total_nodes = %self.nodes.len(),
            "Vector inserted successfully"
        );
        Ok(())
    }

    async fn search(&self, query: &[f32], k: usize) -> Result<Vec<(Vector, f32)>> {
        debug!(
            k = k,
            dimensions = query.len(),
            "Searching for nearest neighbors"
        );

        if self.nodes.is_empty() {
            warn!("Search called on empty index");
            return Ok(vec![]);
        }

        let entry_point = self.entry_point.as_ref().ok_or_else(|| {
            VectorStoreError::Storage("No entry point found".to_string())
        })?;

        let results = search(
            query,
            k,
            entry_point,
            &self.nodes,
            &self.config,
            &*self.metric,
        )?;

        debug!(
            results_count = results.len(),
            "Search completed successfully"
        );

        Ok(results
            .into_iter()
            .map(|(id, distance)| {
                let node = self.nodes.get(&id).unwrap();
                (
                    Vector {
                        id: id.clone(),
                        data: node.data.clone(),
                        metadata: None,
                    },
                    distance,
                )
            })
            .collect())
    }

    async fn get(&self, id: &str) -> Result<Option<Vector>> {
        debug!(vector_id = %id, "Getting vector by ID");
        
        match self.nodes.get(id) {
            Some(node) => {
                debug!("Vector found");
                Ok(Some(Vector {
                    id: id.to_string(),
                    data: node.data.clone(),
                    metadata: None,
                }))
            }
            None => {
                debug!("Vector not found");
                Ok(None)
            }
        }
    }

    async fn delete(&mut self, id: &str) -> Result<()> {
        debug!(vector_id = %id, "Deleting vector");
        
        if self.nodes.remove(id).is_some() {
            info!(vector_id = %id, "Vector deleted successfully");
            
            if Some(id.to_string()) == self.entry_point {
                warn!(
                    vector_id = %id,
                    "Deleted entry point, selecting new one if available"
                );
                self.entry_point = self.nodes.keys().next().map(|k| k.to_string());
            }
            
            Ok(())
        } else {
            warn!(vector_id = %id, "Attempted to delete non-existent vector");
            Err(VectorStoreError::NotFound(id.to_string()))
        }
    }

    async fn len(&self) -> Result<usize> {
        debug!(count = %self.nodes.len(), "Getting index size");
        Ok(self.nodes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::metrics::EuclideanDistance;

    #[tokio::test]
    async fn test_empty_index() {
        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let index = HnswIndex::new(config, Box::new(EuclideanDistance));
        
        assert_eq!(index.len().await.unwrap(), 0);
        assert!(index.get("non_existent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_single_insert() {
        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let mut index = HnswIndex::new(config, Box::new(EuclideanDistance));
        
        let vector = Vector {
            id: "test".to_string(),
            data: vec![1.0, 0.0],
            metadata: None,
        };
        
        index.insert(vector.clone()).await.unwrap();
        assert_eq!(index.len().await.unwrap(), 1);
        
        let retrieved = index.get("test").await.unwrap().unwrap();
        assert_eq!(retrieved.id, "test");
        assert_eq!(retrieved.data, vec![1.0, 0.0]);
    }

    #[tokio::test]
    async fn test_search() {
        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let mut index = HnswIndex::new(config, Box::new(EuclideanDistance));
        
        // Insert some test vectors
        for i in 0..5 {
            let vector = Vector {
                id: format!("test_{}", i),
                data: vec![i as f32, 0.0],
                metadata: None,
            };
            index.insert(vector).await.unwrap();
        }
        
        // Search for nearest neighbors
        let results = index.search(&[2.0, 0.0], 3).await.unwrap();
        assert_eq!(results.len(), 3);
        
        // The closest vector should be test_2
        assert_eq!(results[0].0.id, "test_2");
    }
}
