use vector_store::{
    core::config::MemoryConfig,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    storage::metrics::CosineDistance,
};
use std::time::SystemTime;
use std::sync::Arc;

#[tokio::test]
async fn test_hnsw_functionality_without_neighbour_import() {
    // Create a memory storage instance
    let config = MemoryConfig {
        max_dimensions: 4,
        ..MemoryConfig::default()
    };
    let distance_metric = Arc::new(CosineDistance::new());
    let mut storage = vector_store::memory::temporal::MemoryStorage::new(
        config,
        distance_metric,
    );

    // Create and save a test vector
    let vector = Vector::new(
        "test_vector_1".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );

    let temporal = TemporalVector::new(
        vector,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 0.8,
            context: "test_context".to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );

    // Save the vector
    storage.save_memory(temporal.clone()).await.expect("Failed to save memory");

    // Search for similar vectors
    let results = storage.search_similar(&[0.1, 0.2, 0.3, 0.4], 1).await.expect("Failed to search");
    
    // Verify results
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.vector.id, "test_vector_1");
}
