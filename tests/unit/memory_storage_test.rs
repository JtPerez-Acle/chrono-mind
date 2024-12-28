#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_memory_storage_basic_operations() {
        let mut storage = MemoryVectorStorage::new();
        
        // Test insert and get
        let vector = Vector {
            id: "test1".to_string(),
            data: vec![1.0, 2.0, 3.0],
            metadata: None,
        };
        
        storage.insert(vector.clone()).await.unwrap();
        assert_eq!(storage.len().await.unwrap(), 1);
        
        let retrieved = storage.get("test1").await.unwrap().unwrap();
        assert_eq!(retrieved.data, vector.data);
        
        // Test delete
        storage.delete("test1").await.unwrap();
        assert_eq!(storage.len().await.unwrap(), 0);
        assert!(storage.get("test1").await.unwrap().is_none());
    }

    #[test(tokio::test)]
    async fn test_vector_search() {
        let mut storage = MemoryVectorStorage::new();
        
        // Insert some test vectors
        let vectors = vec![
            Vector {
                id: "1".to_string(),
                data: vec![1.0, 0.0, 0.0],
                metadata: None,
            },
            Vector {
                id: "2".to_string(),
                data: vec![0.0, 1.0, 0.0],
                metadata: None,
            },
            Vector {
                id: "3".to_string(),
                data: vec![0.0, 0.0, 1.0],
                metadata: None,
            },
        ];
        
        for v in vectors {
            storage.insert(v).await.unwrap();
        }
        
        // Search for nearest vector to [1.0, 0.0, 0.0]
        let results = storage.search(&[1.0, 0.0, 0.0], 1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "1");
        assert_eq!(results[0].1, 0.0); // Distance should be 0 for exact match
    }
}
