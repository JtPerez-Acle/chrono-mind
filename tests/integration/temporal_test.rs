use std::time::SystemTime;

use vector_store::{
    core::config::MemoryConfig,
    memory::{
        temporal::MemoryStorage,
        types::{MemoryAttributes, TemporalVector, Vector},
    },
    storage::metrics::CosineDistance,
    utils::validation::{validate_dimensions, validate_temporal_vector},
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
async fn test_memory_operations() {
    let config = MemoryConfig::default();
    let metric = CosineDistance::new();
    let mut storage = MemoryStorage::new(metric, config.clone()).unwrap();

    // Test vector creation and validation
    let v1 = create_test_vector("1", vec![1.0, 0.0], 1.0);
    assert!(validate_temporal_vector(&v1).is_ok());
    assert!(validate_dimensions(&v1.vector, &config).is_ok());

    // Test memory storage
    storage.save_memory(v1.clone()).await.unwrap();
    let retrieved = storage.get_memory("1").await.unwrap().unwrap();
    assert_eq!(retrieved.vector.id, "1");
    assert_eq!(retrieved.vector.data, vec![1.0, 0.0]);

    // Test similar memory search
    let v2 = create_test_vector("2", vec![0.8, 0.2], 1.0);
    storage.save_memory(v2).await.unwrap();

    let query = vec![1.0, 0.0];
    let similar = storage.search_similar(&query, 2).await.unwrap();
    assert_eq!(similar.len(), 2);
    assert_eq!(similar[0].0.vector.id, "1"); // Most similar
}

#[tokio::test]
async fn test_temporal_decay() {
    let config = MemoryConfig {
        base_decay_rate: 0.5,
        min_importance: 0.1,
        max_importance: 1.0,
        ..Default::default()
    };
    let metric = CosineDistance::new();
    let mut storage = MemoryStorage::new(metric, config.clone()).unwrap();

    // Add memory with different timestamps and access patterns
    let mut v1 = create_test_vector("1", vec![1.0], 1.0);
    let mut v2 = create_test_vector("2", vec![1.0], 1.0);

    // Set v1 to be older with low access count
    v1.attributes.timestamp = SystemTime::now() - std::time::Duration::from_secs(3600); // 1 hour ago
    v1.attributes.last_access = v1.attributes.timestamp;
    v1.attributes.access_count = 1;

    // Set v2 to be newer with high access count
    v2.attributes.access_count = 5;
    v2.attributes.last_access = SystemTime::now();
    
    storage.save_memory(v1.clone()).await.unwrap();
    storage.save_memory(v2.clone()).await.unwrap();

    // Update decay
    storage.update_memory_decay().await.unwrap();

    // Get memories and compare importance
    let m1 = storage.get_memory("1").await.unwrap().unwrap();
    let m2 = storage.get_memory("2").await.unwrap().unwrap();
    
    assert!(m1.attributes.importance < m2.attributes.importance,
        "Expected older memory (m1) to have lower importance.\n\
         m1: age=1h, access_count={}, importance={:.3}\n\
         m2: age=0h, access_count={}, importance={:.3}",
        m1.attributes.access_count, m1.attributes.importance,
        m2.attributes.access_count, m2.attributes.importance);
}

#[tokio::test]
async fn test_context_operations() {
    let config = MemoryConfig::default();
    let metric = CosineDistance::new();
    let mut storage = MemoryStorage::new(metric, config.clone()).unwrap();

    // Add memories with different contexts
    let mut v1 = create_test_vector("1", vec![1.0], 1.0);
    v1.attributes.context = "context1".to_string();
    storage.save_memory(v1).await.unwrap();

    let mut v2 = create_test_vector("2", vec![1.0], 1.0);
    v2.attributes.context = "context2".to_string();
    storage.save_memory(v2).await.unwrap();

    // Test context retrieval
    let context1_memories = storage.get_memories_by_context("context1").await.unwrap();
    assert_eq!(context1_memories.len(), 1);
    assert_eq!(context1_memories[0].vector.id, "1");

    // Test context summary
    let summary = storage.get_context_summary("context1").await.unwrap();
    assert_eq!(summary.context, "context1");
    assert_eq!(summary.memory_count, 1);
}

#[tokio::test]
async fn test_relationship_tracking() {
    let config = MemoryConfig::default();
    let metric = CosineDistance::new();
    let mut storage = MemoryStorage::new(metric, config.clone()).unwrap();

    // Create memories with relationships
    let mut v1 = create_test_vector("1", vec![1.0], 1.0);
    let mut v2 = create_test_vector("2", vec![1.0], 1.0);
    v1.attributes.relationships.push("2".to_string());
    v2.attributes.relationships.push("1".to_string());

    storage.save_memory(v1).await.unwrap();
    storage.save_memory(v2).await.unwrap();

    // Test relationship retrieval
    let relationships = storage.get_relationships("1").await.unwrap();
    assert_eq!(relationships.len(), 1);
    assert_eq!(relationships[0].vector.id, "2");
}

#[tokio::test]
async fn test_memory_stats() {
    let config = MemoryConfig::default();
    let metric = CosineDistance::new();
    let mut storage = MemoryStorage::new(metric, config.clone()).unwrap();

    // Add memories
    let v1 = create_test_vector("1", vec![1.0], 1.0);
    let v2 = create_test_vector("2", vec![1.0], 0.5);
    storage.save_memory(v1).await.unwrap();
    storage.save_memory(v2).await.unwrap();

    // Get stats
    let stats = storage.get_memory_stats().await.unwrap();
    assert_eq!(stats.total_memories, 2);
    assert!(stats.average_importance > 0.0);
    assert!(stats.capacity_used > 0.0);
}
