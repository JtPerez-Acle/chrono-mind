use vector_store::{
    core::config::MemoryConfig,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    storage::persistence::{MemoryBackend, StorageBackend},
};
use std::time::SystemTime;

#[tokio::test]
async fn test_memory_backend_count() {
    // Create test data with custom dimensions
    let config = MemoryConfig {
        max_dimensions: 4,
        ..MemoryConfig::default()
    };
    let mut backend = MemoryBackend::new(config.clone());

    // Initialize the backend
    backend.init().await.expect("Failed to initialize backend");

    // Create and save test vectors
    for i in 1..=5 {
        let vector = Vector::new(
            format!("test_vector_{}", i),
            vec![0.1 * i as f32, 0.2 * i as f32, 0.3 * i as f32, 0.4 * i as f32],
        );

        let temporal = TemporalVector::new(
            vector,
            MemoryAttributes {
                timestamp: SystemTime::now(),
                importance: 0.5 + (i as f32 * 0.1),
                context: format!("context_{}", i % 3),
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: i,
                last_access: SystemTime::now(),
            },
        );

        backend.save(&temporal).await.expect("Failed to save memory");
    }

    // Test the count method via the trait
    let count_via_trait = <MemoryBackend as StorageBackend>::count(&backend).await.expect("Failed to get count via trait");
    assert_eq!(count_via_trait, 5);

    // Test the direct count method
    let direct_count = backend.count().await;
    assert_eq!(direct_count, 5);

    // Add more vectors
    for i in 6..=10 {
        let vector = Vector::new(
            format!("test_vector_{}", i),
            vec![0.1 * i as f32, 0.2 * i as f32, 0.3 * i as f32, 0.4 * i as f32],
        );

        let temporal = TemporalVector::new(
            vector,
            MemoryAttributes {
                timestamp: SystemTime::now(),
                importance: 0.5 + (i as f32 * 0.1),
                context: format!("context_{}", i % 3),
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: i,
                last_access: SystemTime::now(),
            },
        );

        backend.save(&temporal).await.expect("Failed to save memory");
    }

    // Test the count method via the trait again
    let count_via_trait = <MemoryBackend as StorageBackend>::count(&backend).await.expect("Failed to get count via trait");
    assert_eq!(count_via_trait, 10);

    // Test the direct count method again
    let direct_count = backend.count().await;
    assert_eq!(direct_count, 10);
}
