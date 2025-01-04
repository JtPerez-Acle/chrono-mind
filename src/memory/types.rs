use std::time::{SystemTime, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub access_count: usize,
    pub last_access: SystemTime,
}

/// A vector with temporal memory attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalVector {
    pub vector: Vector,
    pub attributes: MemoryAttributes,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub access_count: usize,
}

impl TemporalVector {
    pub fn new(vector: Vector, attributes: MemoryAttributes) -> Self {
        Self {
            vector,
            attributes,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            access_count: 0,
        }
    }

    pub fn validate(&self) -> bool {
        // Vector data should not be empty
        if self.vector.data.is_empty() {
            return false;
        }

        // Vector ID should not be empty
        if self.vector.id.is_empty() {
            return false;
        }

        // Importance should be between 0 and 1
        if self.attributes.importance < 0.0 || self.attributes.importance > 1.0 {
            return false;
        }

        // Context should not be empty
        if self.attributes.context.is_empty() {
            return false;
        }

        true
    }

    pub fn update_access(&mut self) {
        self.last_accessed = SystemTime::now();
        self.access_count += 1;
    }

    pub fn get_age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or_default()
    }

    pub fn get_last_access_age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.last_accessed)
            .unwrap_or_default()
    }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_memories: usize,
    pub total_size: usize,
    pub avg_vector_size: f64,
    pub capacity_used: f64,
    pub average_importance: f32,
    pub context_distribution: HashMap<String, usize>,
    pub most_connected_memories: Vec<String>,
}
