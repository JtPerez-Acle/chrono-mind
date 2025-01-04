use std::{
    sync::Arc,
    time::{SystemTime, Duration},
};
use vector_store::{
    core::error::Result,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    storage::{
        metrics::CosineDistance,
        hnsw::{HNSWConfig, TemporalHNSW},
    },
};

fn create_test_vector(id: &str, vec: Vec<f32>, importance: f32) -> TemporalVector {
    let now = SystemTime::now();
    let vector = Vector::new(id.to_string(), vec);
    let attributes = MemoryAttributes {
        timestamp: now,
        importance,
        context: "test".to_string(),
        decay_rate: 0.1,
        relationships: vec![],
        access_count: 0,
        last_access: now,
    };
    TemporalVector::new(vector, attributes)
}

fn create_test_vector_with_time(id: &str, vec: Vec<f32>, importance: f32, timestamp: SystemTime) -> TemporalVector {
    let vector = Vector::new(id.to_string(), vec);
    let attributes = MemoryAttributes {
        timestamp,
        importance,
        context: "test".to_string(),
        decay_rate: 0.1,
        relationships: vec![],
        access_count: 0,
        last_access: timestamp,
    };
    TemporalVector::new(vector, attributes)
}

#[tokio::test]
async fn test_basic_insert_search() -> Result<()> {
    let config = HNSWConfig {
        ef_construction: 10,
        ef_search: 10,
        max_dimensions: 3,
        temporal_weight: 0.1,
        max_connections: 16,
    };
    let metric = Arc::new(CosineDistance::new());
    let index = TemporalHNSW::new(config, metric);

    let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);
    let v3 = create_test_vector("3", vec![0.0, 0.0, 1.0], 1.0);

    index.insert(&v1).await?;
    index.insert(&v2).await?;
    index.insert(&v3).await?;

    let query = vec![1.0, 0.0, 0.0];
    let results = index.search(&query, 3).await?;

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0, "1");  // Most similar to query
    assert_eq!(results[1].0, "2");  // Second most similar
    assert_eq!(results[2].0, "3");  // Least similar

    Ok(())
}

#[tokio::test]
async fn test_temporal_ordering() -> Result<()> {
    let config = HNSWConfig {
        ef_construction: 10,
        ef_search: 10,
        max_dimensions: 3,
        temporal_weight: 0.8,  // High temporal weight to prioritize recency
        max_connections: 16,
    };
    let metric = Arc::new(CosineDistance::new());
    let index = TemporalHNSW::new(config, metric);

    let now = SystemTime::now();
    let v1 = create_test_vector_with_time(
        "1",
        vec![1.0, 0.0, 0.0],
        1.0,
        now - Duration::from_secs(10),
    );
    let v2 = create_test_vector_with_time(
        "2",
        vec![0.0, 1.0, 0.0],
        1.0,
        now,
    );

    index.insert(&v1).await?;
    index.insert(&v2).await?;

    let query = vec![1.0, 0.0, 0.0];
    let results = index.search(&query, 2).await?;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "2");  // Should be first due to recency
    assert_eq!(results[1].0, "1");

    Ok(())
}

#[tokio::test]
async fn test_empty_index() -> Result<()> {
    let config = HNSWConfig {
        ef_construction: 10,
        ef_search: 10,
        max_dimensions: 3,
        temporal_weight: 0.1,
        max_connections: 16,
    };
    let metric = Arc::new(CosineDistance::new());
    let index = TemporalHNSW::new(config, metric);

    let query = vec![1.0, 0.0, 0.0];
    let results = index.search(&query, 1).await?;
    assert_eq!(results.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_dimension_validation() -> Result<()> {
    let config = HNSWConfig {
        ef_construction: 10,
        ef_search: 10,
        max_dimensions: 3,
        temporal_weight: 0.1,
        max_connections: 16,
    };
    let metric = Arc::new(CosineDistance::new());
    let index = TemporalHNSW::new(config, metric);

    let v1 = create_test_vector("1", vec![1.0, 0.0], 1.0);
    assert!(index.insert(&v1).await.is_err());

    let v2 = create_test_vector("2", vec![1.0, 0.0, 0.0, 0.0], 1.0);
    assert!(index.insert(&v2).await.is_err());

    Ok(())
}

#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    let config = HNSWConfig {
        ef_construction: 10,
        ef_search: 10,
        max_dimensions: 3,
        temporal_weight: 0.1,
        max_connections: 16,
    };
    let metric = Arc::new(CosineDistance::new());
    let index = Arc::new(TemporalHNSW::new(config, metric));

    let mut handles = Vec::new();
    for i in 0..10 {
        let index = index.clone();
        let handle = tokio::spawn(async move {
            let v = create_test_vector(
                &format!("{}", i),
                vec![1.0, 0.0, 0.0],
                1.0,
            );
            index.insert(&v).await
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;  // Using the new From<JoinError> implementation
    }

    let query = vec![1.0, 0.0, 0.0];
    let results = index.search(&query, 5).await?;
    assert_eq!(results.len(), 5);

    Ok(())
}

#[tokio::test]
async fn test_layer_stats() -> Result<()> {
    let config = HNSWConfig {
        ef_construction: 10,
        ef_search: 10,
        max_dimensions: 3,
        temporal_weight: 0.1,
        max_connections: 16,
    };
    let metric = Arc::new(CosineDistance::new());
    let index = TemporalHNSW::new(config, metric);

    let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);
    let v3 = create_test_vector("3", vec![0.0, 0.0, 1.0], 1.0);

    index.insert(&v1).await?;
    index.insert(&v2).await?;
    index.insert(&v3).await?;

    let stats = index.get_layer_stats().await?;
    assert!(stats.total_nodes > 0);
    assert!(stats.total_connections >= 0);
    assert!(stats.avg_connections >= 0.0);

    Ok(())
}
