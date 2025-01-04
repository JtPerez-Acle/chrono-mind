use std::time::SystemTime;

use vector_store::{
    MemoryAttributes, TemporalVector, Vector,
    CosineDistance, HNSWConfig, TemporalHNSW,
};

fn create_test_vector(id: &str, data: Vec<f32>, importance: f32) -> TemporalVector {
    TemporalVector {
        vector: Vector {
            id: id.to_string(),
            data,
        },
        attributes: MemoryAttributes {
            timestamp: SystemTime::now(),
            importance,
            context: "test".to_string(),
            decay_rate: 0.1,
            relationships: vec![],
            access_count: 0,
            last_access: SystemTime::now(),
        },
    }
}

#[tokio::test]
async fn test_hnsw_basic_operations() {
    let config = HNSWConfig::default();
    let metric = CosineDistance::new();
    let index = TemporalHNSW::new(metric, config);

    // Test insertion
    let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);
    let v3 = create_test_vector("3", vec![0.0, 0.0, 1.0], 1.0);

    index.insert(&v1).await.unwrap();
    index.insert(&v2).await.unwrap();
    index.insert(&v3).await.unwrap();

    // Test search
    let query = vec![1.0, 0.1, 0.1];
    let results = index.search(&query, 2).await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "1"); // Closest to query
}

#[tokio::test]
async fn test_hnsw_temporal_awareness() {
    let config = HNSWConfig {
        temporal_weight: 0.5,
        ..Default::default()
    };
    let metric = CosineDistance::new();
    let index = TemporalHNSW::new(metric, config);

    // Create vectors with different temporal properties
    let v1 = create_test_vector("1", vec![1.0, 0.0], 1.0);
    let mut v2 = create_test_vector("2", vec![0.9, 0.1], 0.5);

    // Make v2 appear older
    v2.attributes.timestamp = SystemTime::now() - std::time::Duration::from_secs(3600);

    index.insert(&v1).await.unwrap();
    index.insert(&v2).await.unwrap();

    // Search with a vector closer to v2
    let query = vec![0.9, 0.1];
    let results = index.search(&query, 2).await.unwrap();

    // Despite being closer in vector space, v2 should rank lower due to temporal score
    assert_eq!(results[0].0, "1");
    assert_eq!(results[1].0, "2");
}

#[tokio::test]
async fn test_hnsw_concurrent_access() {
    let config = HNSWConfig::default();
    let metric = CosineDistance::new();
    let index = std::sync::Arc::new(TemporalHNSW::new(metric, config));

    let mut handles = Vec::new();

    // Spawn multiple tasks to insert vectors
    for i in 0..10 {
        let index = index.clone();
        let handle = tokio::spawn(async move {
            let v = create_test_vector(
                &i.to_string(),
                vec![i as f32 / 10.0, 1.0 - i as f32 / 10.0],
                1.0,
            );
            index.insert(&v).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all insertions to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify search still works
    let query = vec![0.5, 0.5];
    let results = index.search(&query, 5).await.unwrap();
    assert_eq!(results.len(), 5);
}

#[tokio::test]
async fn test_hnsw_edge_cases() {
    let config = HNSWConfig::default();
    let metric = CosineDistance::new();
    let index = TemporalHNSW::new(metric, config);

    // Test empty index
    let query = vec![1.0, 0.0];
    let results = index.search(&query, 1).await.unwrap();
    assert!(results.is_empty());

    // Test single vector
    let v1 = create_test_vector("1", vec![1.0, 0.0], 1.0);
    index.insert(&v1).await.unwrap();
    let results = index.search(&query, 1).await.unwrap();
    assert_eq!(results.len(), 1);

    // Test with zero vector
    let v2 = create_test_vector("2", vec![0.0, 0.0], 1.0);
    index.insert(&v2).await.unwrap();
    let results = index.search(&query, 2).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "1"); // Non-zero vector should be closer

    // Test with duplicate vectors
    let v3 = create_test_vector("3", vec![1.0, 0.0], 1.0);
    index.insert(&v3).await.unwrap();
    let results = index.search(&query, 3).await.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_hnsw_large_scale() {
    let config = HNSWConfig {
        ef_construction: 50,
        max_connections: 16,
        ..Default::default()
    };
    let metric = CosineDistance::new();
    let index = TemporalHNSW::new(metric, config);

    // Insert 1000 random vectors
    for i in 0..1000 {
        let data = vec![
            rand::random::<f32>(),
            rand::random::<f32>(),
            rand::random::<f32>(),
        ];
        let v = create_test_vector(&i.to_string(), data, 1.0);
        index.insert(&v).await.unwrap();
    }

    // Test search performance
    let query = vec![0.5, 0.5, 0.5];
    let start = std::time::Instant::now();
    let results = index.search(&query, 10).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 10);
    assert!(duration.as_millis() < 100); // Search should be fasts
}
