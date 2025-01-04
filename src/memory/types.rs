use std::time::SystemTime;
use serde::{Deserialize, Serialize};

/// Core vector type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vector {
    pub id: String,
    pub data: Vec<f32>,
}

impl Vector {
    pub fn new(id: String, data: Vec<f32>) -> Self {
        Self { id, data }
    }
}

/// Temporal attributes for memory vectors
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

/// A vector with temporal memory attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalVector {
    pub vector: Vector,
    pub attributes: MemoryAttributes,
}

/// Summary of memories in a context
#[derive(Debug, Serialize)]
pub struct ContextSummary {
    pub context: String,
    pub importance: f32,
    pub memory_count: usize,
    pub average_vector: Vec<f32>,
    pub key_relationships: Vec<String>,
}

/// Statistics about the memory storage
#[derive(Debug, Serialize)]
pub struct MemoryStats {
    pub total_memories: usize,
    pub capacity_used: f32,
    pub average_importance: f32,
    pub context_distribution: std::collections::HashMap<String, usize>,
    pub most_connected_memories: Vec<(String, usize)>,
}
