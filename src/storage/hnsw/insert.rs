use rand::random;
use tracing::debug;

use crate::error::{Result, VectorStoreError};
use crate::storage::hnsw::node::Node;
use crate::storage::metrics::DistanceMetric;
use crate::storage::hnsw::Config;
use crate::storage::hnsw::search::search;

impl super::HnswIndex {
    /// Inserts a vector into the index
    pub fn insert(&mut self, id: String, data: Vec<f32>) -> Result<()> {
        debug!(id = %id, "Inserting vector into HNSW index");

        let (id, layer) = self.insert_node(id, data)?;

        // Update entry point if necessary
        if layer > self.nodes[&self.entry_point.clone().unwrap()].layer {
            let old_entry = self.entry_point.clone().unwrap();
            self.entry_point = Some(id.clone());
            debug!(old_entry = %old_entry, new_entry = %id, "Updated entry point");
        }

        debug!(id = %id, "Completing insertion");
        Ok(())
    }

    fn insert_node(
        &mut self,
        id: String,
        data: Vec<f32>,
    ) -> Result<(String, usize), VectorStoreError> {
        insert_node(
            &mut self.nodes,
            id,
            data,
            &self.config,
            &self.metric,
            self.entry_point.clone(),
        )
    }
}

pub fn insert_node(
    nodes: &mut HashMap<String, Node>,
    id: String,
    data: Vec<f32>,
    config: &Config,
    metric: &dyn DistanceMetric,
    entry_point: Option<String>,
) -> Result<(String, usize)> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Determine insertion layer
    let mut layer = 0;
    while rng.gen::<f32>() < 1.0 / config.m as f32 && layer < config.max_layers - 1 {
        layer += 1;
    }

    // Create new node
    let mut node = Node::new(id.clone(), data.clone(), config.max_layers, layer);

    // If this is the first node, just insert it
    if nodes.is_empty() {
        nodes.insert(id.clone(), node);
        return Ok((id, layer));
    }

    // Get entry point
    let mut ep = entry_point.ok_or_else(|| {
        crate::error::VectorStoreError::Storage("No entry point available".to_string())
    })?;

    // For each layer from top to bottom
    for l in (0..=layer).rev() {
        // Search for neighbors at current layer
        let neighbors = search(
            &data,
            config.m,
            &ep,
            nodes,
            config,
            metric,
        )?;

        // Update entry point if we found a better one
        if let Some((neighbor_id, _)) = neighbors.first() {
            ep = neighbor_id.clone();
        }

        // Add connections to M closest neighbors
        let node = nodes.get_mut(&id).unwrap();
        for (neighbor_id, _) in neighbors.iter().take(config.m) {
            node.add_connection(l, neighbor_id.clone());
            
            // Add reverse connection
            if let Some(neighbor) = nodes.get_mut(neighbor_id) {
                neighbor.add_connection(l, id.clone());
            }
        }
    }

    Ok((id, layer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::metrics::EuclideanDistance;

    #[test]
    fn test_insert_first_node() {
        let mut nodes = HashMap::new();
        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let metric = EuclideanDistance;

        let (id, layer) = insert_node(
            &mut nodes,
            "test".to_string(),
            vec![1.0, 0.0],
            &config,
            &metric,
            None,
        ).unwrap();

        assert_eq!(id, "test");
        assert!(layer < config.max_layers);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn test_insert_with_connections() {
        let mut nodes = HashMap::new();
        let config = Config {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        };
        let metric = EuclideanDistance;

        // Insert first node
        let (id1, _) = insert_node(
            &mut nodes,
            "test1".to_string(),
            vec![1.0, 0.0],
            &config,
            &metric,
            None,
        ).unwrap();

        // Insert second node
        let (id2, _) = insert_node(
            &mut nodes,
            "test2".to_string(),
            vec![2.0, 0.0],
            &config,
            &metric,
            Some(id1.clone()),
        ).unwrap();

        // Check connections
        let node1 = nodes.get(&id1).unwrap();
        let node2 = nodes.get(&id2).unwrap();

        assert!(node1.get_connections(0).unwrap().contains(&id2));
        assert!(node2.get_connections(0).unwrap().contains(&id1));
    }
}
