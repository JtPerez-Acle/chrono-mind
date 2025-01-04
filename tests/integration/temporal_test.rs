use std::{
    sync::Arc,
    time::{SystemTime, Duration},
};
use vector_store::{
    core::{
        config::MemoryConfig,
        error::{MemoryError, Result},
    },
    memory::{
        temporal::MemoryStorage,
        types::{MemoryAttributes, TemporalVector, Vector},
    },
    storage::metrics::{CosineDistance, DistanceMetric},
};

// Vector Creation Utilities
mod test_utils {
    use super::*;

    pub fn create_test_vector(id: &str, vec: Vec<f32>, importance: f32) -> TemporalVector {
        let now = SystemTime::now();
        create_test_vector_with_time(id, vec, importance, now)
    }

    pub fn create_test_vector_with_time(
        id: &str,
        vec: Vec<f32>,
        importance: f32,
        timestamp: SystemTime,
    ) -> TemporalVector {
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

    pub fn create_test_vector_with_context(
        id: &str,
        vec: Vec<f32>,
        importance: f32,
        context: &str,
    ) -> TemporalVector {
        let now = SystemTime::now();
        let vector = Vector::new(id.to_string(), vec);
        let attributes = MemoryAttributes {
            timestamp: now,
            importance,
            context: context.to_string(),
            decay_rate: 0.1,
            relationships: vec![],
            access_count: 0,
            last_access: now,
        };
        TemporalVector::new(vector, attributes)
    }

    pub fn validate_vector_dimensions(vector: &TemporalVector, expected_dim: usize) -> Result<()> {
        if vector.vector.data.len() != expected_dim {
            return Err(MemoryError::InvalidDimensions {
                got: vector.vector.data.len(),
                expected: expected_dim,
            });
        }
        Ok(())
    }
}

// Basic Vector Operations Tests
mod vector_operations {
    use super::*;
    use super::test_utils::*;

    #[tokio::test]
    async fn test_basic_vector_operations() -> Result<()> {
        let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 0.8);
        let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 0.6);

        // Test vector properties
        assert_eq!(v1.vector.id, "1");
        assert_eq!(v2.vector.id, "2");
        assert_eq!(v1.vector.data, vec![1.0, 0.0, 0.0]);
        assert_eq!(v2.vector.data, vec![0.0, 1.0, 0.0]);

        Ok(())
    }

    #[tokio::test]
    async fn test_vector_dimensions() -> Result<()> {
        // Test invalid dimensions
        let result = create_test_vector("1", vec![1.0], 0.8);
        let err = validate_vector_dimensions(&result, 3).unwrap_err();
        assert!(matches!(err, MemoryError::InvalidDimensions { .. }));

        // Test empty vector
        let result = create_test_vector("2", vec![], 0.8);
        let err = validate_vector_dimensions(&result, 3).unwrap_err();
        assert!(matches!(err, MemoryError::InvalidDimensions { .. }));

        // Test valid dimensions
        let result = create_test_vector("3", vec![1.0, 0.0, 0.0], 0.8);
        assert!(validate_vector_dimensions(&result, 3).is_ok());

        Ok(())
    }
}

// Temporal Operations Tests
mod temporal_operations {
    use super::*;
    use super::test_utils::*;

    #[tokio::test]
    async fn test_temporal_attributes() -> Result<()> {
        let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 0.8);
        let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 0.6);

        // Test temporal properties
        assert!(v1.attributes.importance > v2.attributes.importance);
        assert_eq!(v1.attributes.context, "test");
        assert_eq!(v2.attributes.context, "test");

        Ok(())
    }

    #[tokio::test]
    async fn test_temporal_ordering() -> Result<()> {
        let now = SystemTime::now();
        let v1 = create_test_vector_with_time(
            "1",
            vec![1.0, 0.0, 0.0],
            0.8,
            now - Duration::from_secs(10),
        );
        let v2 = create_test_vector_with_time("2", vec![0.9, 0.1, 0.0], 0.6, now);

        assert!(v2.attributes.timestamp > v1.attributes.timestamp);
        assert_eq!(v2.attributes.last_access, v2.attributes.timestamp);
        assert_eq!(v1.attributes.last_access, v1.attributes.timestamp);

        Ok(())
    }
}

// Distance Metric Tests
mod distance_metrics {
    use super::*;
    use super::test_utils::*;

    #[tokio::test]
    async fn test_cosine_distance() -> Result<()> {
        let metric = CosineDistance::new();
        let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 0.8);
        let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 0.6);
        let v3 = create_test_vector("3", vec![1.0, 0.0, 0.0], 0.4);

        // Test distance calculations
        let d1 = metric.calculate_distance(&v1.vector.data, &v2.vector.data);
        let d2 = metric.calculate_distance(&v1.vector.data, &v3.vector.data);
        let d3 = metric.calculate_distance(&v2.vector.data, &v3.vector.data);

        assert!(d1 > 0.0); // Orthogonal vectors
        assert_eq!(d2, 0.0); // Same direction
        assert!(d3 > 0.0); // Orthogonal vectors

        Ok(())
    }
}

// Relationship Tests
mod relationship_tests {
    use super::*;
    use super::test_utils::*;

    #[tokio::test]
    async fn test_vector_relationships() -> Result<()> {
        let v1 = create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
        let v2 = create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);
        let v3 = create_test_vector("3", vec![0.0, 0.0, 1.0], 1.0);

        // Test relationship tracking
        assert!(v1.attributes.relationships.is_empty());
        assert!(v2.attributes.relationships.is_empty());
        assert!(v3.attributes.relationships.is_empty());

        Ok(())
    }
}

#[tokio::test]
async fn test_memory_storage_basic() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        min_importance: 0.0,
        max_importance: 1.0,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    let v1 = test_utils::create_test_vector("1", vec![1.0, 0.0, 0.0], 0.8);
    let v2 = test_utils::create_test_vector("2", vec![0.0, 1.0, 0.0], 0.6);
    let v3 = test_utils::create_test_vector("3", vec![0.0, 0.0, 1.0], 0.4);

    storage.save_memory(v1.clone()).await?;
    storage.save_memory(v2.clone()).await?;
    storage.save_memory(v3.clone()).await?;

    let query = vec![1.0, 0.0, 0.0];
    let results = storage.search_similar(&query, 2).await?;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0.vector.id, v1.vector.id);
    assert_eq!(results[1].0.vector.id, v2.vector.id);

    Ok(())
}

#[tokio::test]
async fn test_memory_storage_temporal() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        min_importance: 0.0,
        max_importance: 1.0,
        base_decay_rate: 0.1,  // Low decay rate - should prioritize distance
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    let now = SystemTime::now();
    let v1 = test_utils::create_test_vector_with_time(
        "1",
        vec![1.0, 0.0, 0.0],
        0.8,
        now - Duration::from_secs(10),
    );
    let v2 = test_utils::create_test_vector_with_time(
        "2",
        vec![0.0, 1.0, 0.0],
        0.6,
        now,
    );

    storage.save_memory(v1.clone()).await?;
    storage.save_memory(v2.clone()).await?;

    let query = vec![1.0, 0.0, 0.0];
    let results = storage.search_similar(&query, 2).await?;
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0.vector.id, v1.vector.id);  // Should be first due to distance
    assert_eq!(results[1].0.vector.id, v2.vector.id);

    Ok(())
}

#[tokio::test]
async fn test_memory_storage_importance() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        min_importance: 0.0,
        max_importance: 1.0,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    let v1 = test_utils::create_test_vector("1", vec![1.0, 0.0, 0.0], 0.8);
    let v2 = test_utils::create_test_vector("2", vec![0.0, 1.0, 0.0], 0.6);
    let v3 = test_utils::create_test_vector("3", vec![0.0, 0.0, 1.0], 0.4);

    storage.save_memory(v1.clone()).await?;
    storage.save_memory(v2.clone()).await?;
    storage.save_memory(v3.clone()).await?;

    let query = vec![0.5, 0.5, 0.5];
    let results = storage.search_similar(&query, 3).await?;
    assert_eq!(results.len(), 3);

    // Check that importance affects ordering
    let importance_order: Vec<_> = results.iter().map(|(m, _)| m.attributes.importance).collect();
    assert!(importance_order.windows(2).all(|w| w[0] >= w[1]));

    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        min_importance: 0.0,
        max_importance: 1.0,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    // Test invalid importance value
    let err = storage
        .save_memory(test_utils::create_test_vector(
            "test",
            vec![0.1, 0.2, 0.3],
            1.5,  // Invalid importance > 1.0
        ))
        .await
        .unwrap_err();

    assert!(matches!(err, MemoryError::InvalidImportance(_)));

    // Test invalid dimensions
    let err = storage
        .save_memory(test_utils::create_test_vector(
            "test",
            vec![0.1, 0.2],  // Wrong dimensions
            0.5,
        ))
        .await
        .unwrap_err();

    assert!(matches!(err, MemoryError::InvalidDimensions { .. }));

    Ok(())
}

#[tokio::test]
async fn test_memory_storage_concurrent() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,  // Match the test vector dimensions
        max_memories: 100,
        min_importance: 0.0,
        max_importance: 1.0,
        base_decay_rate: 0.1,
        similarity_threshold: 0.8,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let storage = Arc::new(tokio::sync::RwLock::new(MemoryStorage::new(config, metric)));
    let mut handles = Vec::new();

    for i in 0..10 {
        let storage = storage.clone();
        let handle = tokio::spawn(async move {
            let vector = test_utils::create_test_vector(
                &i.to_string(),
                vec![i as f32 / 10.0, 1.0 - i as f32 / 10.0, 0.5],
                1.0,
            );
            storage.write().await.save_memory(vector).await.unwrap();

            // Perform a search operation while other tasks are writing
            let query = vec![0.5, 0.5, 0.5];
            storage.read().await.search_similar(&query, 5).await.unwrap()
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final state
    let final_count = storage.read().await.get_memory_count().await;
    assert_eq!(final_count, 10);

    Ok(())
}

#[tokio::test]
async fn test_temporal_decay() {
    let config = MemoryConfig {
        max_dimensions: 3,
        max_memories: 100,
        min_importance: 0.1,
        max_importance: 1.0,
        base_decay_rate: 0.5,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    // Add memories with different timestamps
    let mut v1 = test_utils::create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let mut v2 = test_utils::create_test_vector("2", vec![1.0, 0.0, 0.0], 1.0);

    // Set v1 to be older with low access count
    v1.attributes.timestamp = SystemTime::now() - Duration::from_secs(3600);
    v1.attributes.last_access = v1.attributes.timestamp;
    v1.attributes.access_count = 1;
    v1.attributes.decay_rate = 0.2;

    // Set v2 to be newer with high access count
    v2.attributes.access_count = 5;
    v2.attributes.last_access = SystemTime::now();
    v2.attributes.decay_rate = 0.1;

    storage.save_memory(v1.clone()).await.unwrap();
    storage.save_memory(v2.clone()).await.unwrap();

    // Update decay
    storage.update_memory_decay().await.unwrap();

    // Get memories and verify decay
    let m1 = storage.get_memory(&v1.vector.id).await.unwrap().unwrap();
    let m2 = storage.get_memory(&v2.vector.id).await.unwrap().unwrap();

    assert!(m1.attributes.importance < m2.attributes.importance);
}

#[tokio::test]
async fn test_context_operations() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        min_importance: 0.0,
        max_importance: 1.0,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    // Add memories with different contexts
    storage
        .save_memory(test_utils::create_test_vector_with_context(
            "1",
            vec![0.1, 0.2, 0.3],
            0.8,
            "context1",
        ))
        .await?;

    storage
        .save_memory(test_utils::create_test_vector_with_context(
            "2",
            vec![0.4, 0.5, 0.6],
            0.9,
            "context1",
        ))
        .await?;

    storage
        .save_memory(test_utils::create_test_vector_with_context(
            "3",
            vec![0.7, 0.8, 0.9],
            0.7,
            "context2",
        ))
        .await?;

    // Search in context1
    let results = storage
        .search_by_context("context1", &vec![0.1, 0.2, 0.3], 5)
        .await?;

    assert_eq!(results.len(), 2);  // Should only return memories from context1

    // Search in context2
    let results = storage
        .search_by_context("context2", &vec![0.1, 0.2, 0.3], 5)
        .await?;

    assert_eq!(results.len(), 1);  // Should only return memories from context2

    Ok(())
}

#[tokio::test]
async fn test_relationship_tracking() -> Result<()> {
    let mut storage = MemoryStorage::new(
        MemoryConfig::default(),
        Arc::new(CosineDistance::new()),
    );

    let v1 = test_utils::create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let v2 = test_utils::create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);
    let v3 = test_utils::create_test_vector("3", vec![0.0, 0.0, 1.0], 1.0);

    storage.save_memory(v1.clone()).await?;
    storage.save_memory(v2.clone()).await?;
    storage.save_memory(v3).await?;

    // Test relationship tracking
    let mut v1_updated = v1.clone();
    v1_updated.attributes.relationships.push(v2.vector.id.clone());
    storage.save_memory(v1_updated).await?;

    let related = storage.get_related_memories(&v1.vector.id, 1).await?;
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].vector.id, v2.vector.id);

    Ok(())
}

#[tokio::test]
async fn test_memory_consolidation() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        max_memories: 100,
        min_importance: 0.0,
        max_importance: 1.0,
        base_decay_rate: 0.1,
        similarity_threshold: 0.8,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    // Add test vectors
    let v1 = test_utils::create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let v2 = test_utils::create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);

    storage.save_memory(v1).await?;
    storage.save_memory(v2).await?;

    // Test consolidation
    storage.consolidate_memories().await?;
    let memories = storage.list_memories().await?;
    assert_eq!(memories.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_temporal_test() -> Result<()> {
    let config = MemoryConfig {
        max_dimensions: 3,
        max_memories: 100,
        min_importance: 0.0,
        max_importance: 1.0,
        base_decay_rate: 0.1,  // Low decay rate - should prioritize distance
        similarity_threshold: 0.8,
        ..Default::default()
    };
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    let v1 = test_utils::create_test_vector("1", vec![1.0, 0.0, 0.0], 1.0);
    let v2 = test_utils::create_test_vector("2", vec![0.0, 1.0, 0.0], 1.0);

    storage.save_memory(v1).await?;
    storage.save_memory(v2).await?;

    let query = vec![1.0, 0.0, 0.0];
    let results = storage.search_similar(&query, 2).await?;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0.vector.id, "1");  // Should be first due to distance
    assert_eq!(results[1].0.vector.id, "2");

    Ok(())
}
