use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node in the HNSW graph
#[derive(Debug, Clone)]
pub struct Node {
    /// Vector ID
    pub id: String,
    /// Vector data
    data: Vec<f32>,
    /// Connections
    pub connections: HashMap<usize, Vec<String>>,
    /// Layer where this node was inserted
    pub max_layer: usize,
}

impl Node {
    pub fn new(id: String, data: Vec<f32>, max_layers: usize, layer: usize) -> Self {
        let mut connections = HashMap::new();
        for l in 0..=layer {
            connections.insert(l, Vec::new());
        }
        
        Self {
            id,
            data,
            connections,
            max_layer: layer,
        }
    }

    pub fn vector(&self) -> &[f32] {
        &self.data
    }

    pub fn add_connection(&mut self, layer: usize, neighbor_id: String) {
        self.connections.entry(layer)
            .or_insert_with(Vec::new)
            .push(neighbor_id);
    }

    pub fn remove_connection(&mut self, layer: usize, neighbor_id: &str) {
        if let Some(connections) = self.connections.get_mut(&layer) {
            connections.retain(|id| id != neighbor_id);
        }
    }

    pub fn get_connections(&self, layer: usize) -> Option<&Vec<String>> {
        self.connections.get(&layer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = Node::new(
            "test".to_string(),
            vec![1.0, 0.0],
            4,
            2,
        );

        assert_eq!(node.id, "test");
        assert_eq!(node.vector(), &[1.0, 0.0]);
        assert_eq!(node.max_layer, 2);
        assert_eq!(node.connections.len(), 3); // layers 0, 1, 2
    }

    #[test]
    fn test_node_connections() {
        let mut node = Node::new(
            "test".to_string(),
            vec![1.0, 0.0],
            4,
            2,
        );

        // Add connections
        node.add_connection(0, "neighbor1".to_string());
        node.add_connection(0, "neighbor2".to_string());
        node.add_connection(1, "neighbor3".to_string());

        // Check connections
        let layer0_connections = node.get_connections(0).unwrap();
        assert_eq!(layer0_connections.len(), 2);
        assert!(layer0_connections.contains(&"neighbor1".to_string()));
        assert!(layer0_connections.contains(&"neighbor2".to_string()));

        let layer1_connections = node.get_connections(1).unwrap();
        assert_eq!(layer1_connections.len(), 1);
        assert!(layer1_connections.contains(&"neighbor3".to_string()));

        // Remove connection
        node.remove_connection(0, "neighbor1");
        let layer0_connections = node.get_connections(0).unwrap();
        assert_eq!(layer0_connections.len(), 1);
        assert!(layer0_connections.contains(&"neighbor2".to_string()));
    }
}
