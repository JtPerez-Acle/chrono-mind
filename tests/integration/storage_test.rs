use vault_rag::{
    error::Result,
    storage::{
        memory::MemoryVectorStorage,
        mmap::MmapVectorStorage,
        metrics::EuclideanDistance,
        Vector, VectorStorage,
    },
};

async fn test_storage_implementation<S>(storage: &mut S) -> Result<()>
where
    S: VectorStorage,
{
    // Test basic operations
    let vector = Vector {
        id: "test1".to_string(),
        data: vec![1.0, 2.0, 3.0],
    };
    storage.insert(vector.clone()).await?;
    
    // Test retrieval
    let retrieved = storage.get(&vector.id).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().data, vector.data);
    
    // Test search
    let results = storage.search(&[1.0, 2.0, 3.0], 1).await?;
    assert!(!results.is_empty());
    
    // Test deletion
    storage.delete(&vector.id).await?;
    let deleted = storage.get(&vector.id).await?;
    assert!(deleted.is_none());
    
    Ok(())
}

#[tokio::test]
async fn test_memory_storage() -> Result<()> {
    let mut storage = MemoryVectorStorage::with_metric(Box::new(EuclideanDistance));
    test_storage_implementation(&mut storage).await
}

#[tokio::test]
async fn test_mmap_storage() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let path = temp_dir.path().join("test.mmap");
    let mut storage = MmapVectorStorage::create(path)?;
    test_storage_implementation(&mut storage).await
}
