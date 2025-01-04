use std::time::{Duration, SystemTime};
use proptest::prelude::*;
use vault_rag::{
    core::config::MemoryConfig,
    memory::types::{Vector, TemporalVector, MemoryAttributes},
};

/// Test configuration with reasonable defaults for testing
pub fn test_config() -> MemoryConfig {
    MemoryConfig {
        max_dimensions: 10,
        max_memories: 100,
        max_relationships: 10,
        base_decay_rate: 0.1,
        consolidation_window: Duration::from_secs(1),
        min_importance: 0.1,
        max_importance: 1.0,
        similar_memory_count: 5,
        similarity_threshold: 0.8,
        max_context_window: 100,
    }
}

/// Generate a random vector with given dimensions
pub fn generate_random_vector(id: &str, dimensions: usize) -> Vector {
    Vector {
        id: id.to_string(),
        data: (0..dimensions)
            .map(|_| rand::random::<f32>())
            .collect(),
    }
}

/// Generate a temporal vector with random attributes
pub fn generate_temporal_vector(id: &str, dimensions: usize, context: &str) -> TemporalVector {
    TemporalVector {
        vector: generate_random_vector(id, dimensions),
        attributes: MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: rand::random::<f32>(),
            context: context.to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    }
}

/// Property-based testing strategies
pub mod strategies {
    use super::*;

    /// Strategy for generating random vectors
    pub fn vector_strategy(dimensions: usize) -> impl Strategy<Value = Vector> {
        let id_strategy = "[a-zA-Z0-9]{1,10}".prop_map(String::from);
        let data_strategy = prop::collection::vec(any::<f32>(), dimensions);

        (id_strategy, data_strategy).prop_map(|(id, data)| Vector { id, data })
    }

    /// Strategy for generating temporal vectors
    pub fn temporal_vector_strategy(
        dimensions: usize,
        contexts: Vec<String>,
    ) -> impl Strategy<Value = TemporalVector> {
        let vector_strat = vector_strategy(dimensions);
        let context_strat = prop::sample::select(contexts);
        let importance_strat = any::<f32>().prop_map(|x| x.abs() % 1.0);
        let decay_strat = any::<f32>().prop_map(|x| (x.abs() % 0.9) + 0.1);
        let relationships_strat = relationship_strategy(10);

        (
            vector_strat,
            context_strat,
            importance_strat,
            decay_strat,
            relationships_strat,
        )
            .prop_map(
                |(vector, context, importance, decay_rate, relationships)| TemporalVector {
                    vector,
                    attributes: MemoryAttributes {
                        timestamp: SystemTime::now(),
                        importance,
                        context,
                        decay_rate,
                        relationships,
                        access_count: 0,
                        last_access: SystemTime::now(),
                    },
                },
            )
    }

    /// Strategy for generating relationship lists
    pub fn relationship_strategy(max_relationships: usize) -> impl Strategy<Value = Vec<String>> {
        let id_strategy = "[a-zA-Z0-9]{1,10}".prop_map(String::from);
        prop::collection::vec(id_strategy, 0..max_relationships)
    }
}

/// Validate a temporal vector's properties
pub fn assert_memory_valid(memory: &TemporalVector) {
    // Vector validation
    assert!(!memory.vector.id.is_empty(), "Vector ID cannot be empty");
    assert!(!memory.vector.data.is_empty(), "Vector data cannot be empty");
    
    // Attributes validation
    assert!(
        memory.attributes.importance >= 0.0 && memory.attributes.importance <= 1.0,
        "Importance must be between 0 and 1"
    );
    assert!(
        memory.attributes.decay_rate > 0.0 && memory.attributes.decay_rate <= 1.0,
        "Decay rate must be between 0 and 1"
    );
    assert!(!memory.attributes.context.is_empty(), "Context cannot be empty");
    
    // Time validation
    assert!(
        memory.attributes.last_access >= memory.attributes.timestamp,
        "Last access time cannot be before creation time"
    );
}
