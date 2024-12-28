use std::collections::HashMap;
use crate::error::Result;
use super::{Vector, VectorStorage};
use super::metrics::{DistanceMetric, EuclideanDistance};
use tracing::{debug, info};

/// In-memory implementation of vector storage
pub struct MemoryVectorStorage {
    vectors: HashMap<String, Vector>,
    metric: Box<dyn DistanceMetric>,
}

impl MemoryVectorStorage {
    pub fn new() -> Self {
        Self::with_metric(Box::new(EuclideanDistance))
    }

    pub fn with_metric(metric: Box<dyn DistanceMetric>) -> Self {
        info!(metric = metric.name(), "Initializing memory vector storage");
        Self {
            vectors: HashMap::new(),
            metric,
        }
    }
}

#[async_trait::async_trait]
impl VectorStorage for MemoryVectorStorage {
    async fn insert(&mut self, vector: Vector) -> Result<()> {
        debug!(id = %vector.id, dimensions = vector.data.len(), "Inserting vector");
        self.vectors.insert(vector.id.clone(), vector);
        Ok(())
    }
    
    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(Vector, f32)>> {
        debug!(dimensions = query.len(), limit = limit, "Searching vectors");
        let mut results: Vec<_> = self.vectors
            .values()
            .map(|v| {
                let distance = self.metric.distance(&v.data, query);
                (v.clone(), distance)
            })
            .collect();
        
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(limit);
        
        debug!(found = results.len(), "Search completed");
        Ok(results)
    }
    
    async fn delete(&mut self, id: &str) -> Result<()> {
        debug!(id = %id, "Deleting vector");
        self.vectors.remove(id);
        Ok(())
    }
    
    async fn get(&self, id: &str) -> Result<Option<Vector>> {
        debug!(id = %id, "Getting vector");
        Ok(self.vectors.get(id).cloned())
    }
    
    async fn len(&self) -> Result<usize> {
        Ok(self.vectors.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;
    use super::super::metrics::{CosineDistance, DotProductDistance};

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
    async fn test_vector_search_with_different_metrics() {
        async fn test_metric(metric: Box<dyn DistanceMetric>) {
            let mut storage = MemoryVectorStorage::with_metric(metric);
            
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
            
            let results = storage.search(&[1.0, 0.0, 0.0], 1).await.unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].0.id, "1");
        }

        test_metric(Box::new(EuclideanDistance)).await;
        test_metric(Box::new(CosineDistance)).await;
        test_metric(Box::new(DotProductDistance)).await;
    }
}
