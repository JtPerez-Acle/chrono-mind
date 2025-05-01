use std::path::PathBuf;
use std::fs;
use tempfile::tempdir;
use vector_store::{
    core::config::MemoryConfig,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    storage::persistence::{PersistentStore, StorageBackend, MemoryBackend},
};
use std::time::SystemTime;

#[tokio::test]
async fn test_persistent_store_save_load() {
    // Create a temporary directory for test files
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("test_store.json");

    // Create test data with custom dimensions
    let config = MemoryConfig {
        max_dimensions: 4, // Set dimensions to match our test vector
        ..MemoryConfig::default()
    };
    let mut store = PersistentStore::new(config.clone());

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

    // Save the vector to the store
    store.save_memory(temporal.clone()).expect("Failed to save memory");

    // Save the store to a file
    store.save_to_file(&file_path).expect("Failed to save to file");

    // Create a new store and load from the file
    let mut loaded_store = PersistentStore::new(config.clone());
    loaded_store.load_from_file(&file_path).expect("Failed to load from file");

    // Verify the loaded data
    let loaded_vector = loaded_store.get_memory("test_vector_1").expect("Failed to get memory");

    assert_eq!(loaded_vector.vector.id, "test_vector_1");
    assert_eq!(loaded_vector.vector.data, vec![0.1, 0.2, 0.3, 0.4]);
    assert_eq!(loaded_vector.attributes.importance, 0.8);
    assert_eq!(loaded_vector.attributes.context, "test_context");
    assert_eq!(loaded_vector.attributes.decay_rate, 0.1);

    // Clean up
    temp_dir.close().expect("Failed to clean up temp directory");
}

#[tokio::test]
async fn test_memory_backend_backup_restore() {
    // Create a temporary directory for test files
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("test_backup.json");

    // Create test data with custom dimensions
    let config = MemoryConfig {
        max_dimensions: 4, // Set dimensions to match our test vectors
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

    // Backup to file
    backend.backup(file_path.clone()).await.expect("Failed to backup");

    // Create a new backend and restore from backup
    let mut new_backend = MemoryBackend::new(config.clone());
    new_backend.restore(file_path).await.expect("Failed to restore from backup");

    // Verify the restored data
    let ids = new_backend.list_ids().await.expect("Failed to list IDs");
    assert_eq!(ids.len(), 5);

    for i in 1..=5 {
        let id = format!("test_vector_{}", i);
        let memory = new_backend.load(&id).await.expect("Failed to load memory").expect("Memory not found");

        assert_eq!(memory.vector.id, id);
        assert_eq!(memory.vector.data, vec![0.1 * i as f32, 0.2 * i as f32, 0.3 * i as f32, 0.4 * i as f32]);
        assert_eq!(memory.attributes.importance, 0.5 + (i as f32 * 0.1));
        assert_eq!(memory.attributes.context, format!("context_{}", i % 3));
        assert_eq!(memory.attributes.access_count, i);
    }

    // Test stats
    let stats = new_backend.get_stats().await.expect("Failed to get stats");
    assert_eq!(stats.total_memories, 5);
    assert_eq!(stats.context_distribution.len(), 3);

    // Clean up
    temp_dir.close().expect("Failed to clean up temp directory");
}

#[tokio::test]
async fn test_persistence_error_handling() {
    // Test loading from non-existent file
    let config = MemoryConfig {
        max_dimensions: 4, // Consistent with other tests
        ..MemoryConfig::default()
    };
    let mut store = PersistentStore::new(config.clone());
    let non_existent_path = PathBuf::from("/non/existent/path/store.json");

    let result = store.load_from_file(&non_existent_path);
    assert!(result.is_err());

    // Test saving to invalid path
    let invalid_path = PathBuf::from("/invalid/path/store.json");
    let result = store.save_to_file(&invalid_path);
    assert!(result.is_err());

    // Test MemoryBackend error handling
    let mut backend = MemoryBackend::new(config.clone());

    // Test deleting non-existent memory
    let result = backend.delete("non_existent_id").await;
    assert!(result.is_err());

    // Test restoring from non-existent backup
    let result = backend.restore(non_existent_path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_persistence_large_dataset() {
    // Create a temporary directory for test files
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let file_path = temp_dir.path().join("large_dataset.json");

    // Create test data with matching dimensions
    let config = MemoryConfig {
        max_dimensions: 64, // Set dimensions to match our test vectors
        max_memories: 1000,
        ..MemoryConfig::default()
    };
    let mut backend = MemoryBackend::new(config.clone());

    // Initialize the backend
    backend.init().await.expect("Failed to initialize backend");

    // Create and save a larger number of test vectors
    for i in 1..=100 {
        let vector_data: Vec<f32> = (0..64).map(|j| (i as f32 * j as f32) / 1000.0).collect();

        let vector = Vector::new(
            format!("large_vector_{}", i),
            vector_data,
        );

        let temporal = TemporalVector::new(
            vector,
            MemoryAttributes {
                timestamp: SystemTime::now(),
                importance: (i as f32) / 100.0,
                context: format!("large_context_{}", i % 5),
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: i % 10,
                last_access: SystemTime::now(),
            },
        );

        backend.save(&temporal).await.expect("Failed to save memory");
    }

    // Backup to file
    backend.backup(file_path.clone()).await.expect("Failed to backup");

    // Verify file exists and has content
    assert!(file_path.exists());
    let metadata = fs::metadata(&file_path).expect("Failed to get file metadata");
    assert!(metadata.len() > 0);

    // Create a new backend and restore from backup
    let mut new_backend = MemoryBackend::new(config.clone());
    new_backend.restore(file_path).await.expect("Failed to restore from backup");

    // Verify the restored data count
    let ids = new_backend.list_ids().await.expect("Failed to list IDs");
    assert_eq!(ids.len(), 100);

    // Get stats and verify
    let stats = new_backend.get_stats().await.expect("Failed to get stats");
    assert_eq!(stats.total_memories, 100);
    assert_eq!(stats.context_distribution.len(), 5);

    // Clean up
    temp_dir.close().expect("Failed to clean up temp directory");
}
