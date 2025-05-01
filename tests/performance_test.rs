#[cfg(test)]
mod performance_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime};
    use vector_store::{
        core::config::MemoryConfig,
        memory::{
            temporal::MemoryStorage,
            types::{MemoryAttributes, TemporalVector, Vector},
        },
        storage::metrics::CosineDistance,
    };

    // Helper function to normalize vector
    fn normalize_vector(vec: &[f32]) -> Vec<f32> {
        let magnitude = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            vec.iter().map(|x| x / magnitude).collect()
        } else {
            vec.to_vec()
        }
    }

    // Generate test vectors
    fn generate_test_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
        let mut vectors = Vec::with_capacity(count);

        for _ in 0..count {
            let vec: Vec<f32> = (0..dim)
                .map(|_| rand::random::<f32>() * 2.0 - 1.0)
                .collect();
            vectors.push(normalize_vector(&vec));
        }

        vectors
    }

    // Measure execution time of an async function
    async fn measure_time_async<F, Fut, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }

    #[tokio::test]
    async fn test_memory_storage_performance() {
        // Test parameters for small dataset
        let vector_count = 100;
        let dim = 64;
        let search_count = 10;

        println!("Running performance test with {} vectors of dimension {}", vector_count, dim);

        // Generate test data
        let test_vectors = generate_test_vectors(vector_count, dim);

        // Initialize storage
        let config = MemoryConfig {
            max_dimensions: dim,
            max_memories: vector_count * 2, // Extra capacity
            ..MemoryConfig::default()
        };
        let metric = Arc::new(CosineDistance::new());
        let mut storage = MemoryStorage::new(config, metric);

        // Measure insertion time
        let (_, insert_time) = measure_time_async(|| async {
            for (i, vec) in test_vectors.iter().enumerate() {
                let vector = Vector::new(
                    format!("vector_{}", i),
                    vec.clone(),
                );

                let temporal = TemporalVector::new(
                    vector,
                    MemoryAttributes {
                        timestamp: SystemTime::now(),
                        importance: 0.5,
                        context: "test_context".to_string(),
                        decay_rate: 0.1,
                        relationships: Vec::new(),
                        access_count: 0,
                        last_access: SystemTime::now(),
                    },
                );

                storage.save_memory(temporal).await.expect("Failed to save memory");
            }
        }).await;

        let per_vector_time = insert_time.div_f32(vector_count as f32);
        println!("Insertion time for {} vectors: {:?} ({:?} per vector)",
            vector_count, insert_time, per_vector_time);

        // Measure search time
        let mut search_times = Vec::with_capacity(search_count);
        for i in 0..search_count {
            let query = &test_vectors[i % test_vectors.len()];
            let (results, search_time) = measure_time_async(|| async {
                storage.search_similar(query, 5).await.expect("Search failed")
            }).await;

            search_times.push(search_time);
            println!("Search {}: {:?}, found {} results", i+1, search_time, results.len());
        }

        // Calculate average search time
        let total_search_time: Duration = search_times.iter().sum();
        let avg_search_time = total_search_time.div_f32(search_count as f32);
        println!("Average search time: {:?}", avg_search_time);

        // Measure memory decay update time
        let (_, decay_time) = measure_time_async(|| async {
            storage.update_memory_decay().await.expect("Failed to update decay")
        }).await;

        println!("Memory decay update time: {:?}", decay_time);

        // Assert reasonable performance
        assert!(per_vector_time < Duration::from_millis(10),
            "Insertion time per vector too high");
        assert!(avg_search_time < Duration::from_millis(50),
            "Average search time too high");
        assert!(decay_time < Duration::from_millis(100),
            "Memory decay update time too high");
    }

    #[tokio::test]
    async fn test_bert_vector_performance() {
        // Test parameters for BERT vectors
        let vector_count = 50;
        let dim = 768; // BERT base dimensions
        let search_count = 5;

        println!("\nRunning BERT vector performance test with {} vectors of dimension {}", vector_count, dim);

        // Generate test data
        let test_vectors = generate_test_vectors(vector_count, dim);

        // Initialize storage
        let config = MemoryConfig {
            max_dimensions: dim,
            max_memories: vector_count * 2,
            ..MemoryConfig::default()
        };
        let metric = Arc::new(CosineDistance::new());
        let mut storage = MemoryStorage::new(config, metric);

        // Measure insertion time
        let (_, insert_time) = measure_time_async(|| async {
            for (i, vec) in test_vectors.iter().enumerate() {
                let vector = Vector::new(
                    format!("bert_vector_{}", i),
                    vec.clone(),
                );

                let temporal = TemporalVector::new(
                    vector,
                    MemoryAttributes {
                        timestamp: SystemTime::now(),
                        importance: 0.5,
                        context: "bert_context".to_string(),
                        decay_rate: 0.1,
                        relationships: Vec::new(),
                        access_count: 0,
                        last_access: SystemTime::now(),
                    },
                );

                storage.save_memory(temporal).await.expect("Failed to save memory");
            }
        }).await;

        let per_vector_time = insert_time.div_f32(vector_count as f32);
        println!("BERT insertion time for {} vectors: {:?} ({:?} per vector)",
            vector_count, insert_time, per_vector_time);

        // Measure search time
        let mut search_times = Vec::with_capacity(search_count);
        for i in 0..search_count {
            let query = &test_vectors[i % test_vectors.len()];
            let (results, search_time) = measure_time_async(|| async {
                storage.search_similar(query, 5).await.expect("Search failed")
            }).await;

            search_times.push(search_time);
            println!("BERT search {}: {:?}, found {} results", i+1, search_time, results.len());
        }

        // Calculate average search time
        let total_search_time: Duration = search_times.iter().sum();
        let avg_search_time = total_search_time.div_f32(search_count as f32);
        println!("BERT average search time: {:?}", avg_search_time);

        // Measure memory decay update time
        let (_, decay_time) = measure_time_async(|| async {
            storage.update_memory_decay().await.expect("Failed to update decay")
        }).await;

        println!("BERT memory decay update time: {:?}", decay_time);

        // Assert reasonable performance for BERT vectors
        assert!(per_vector_time < Duration::from_millis(20),
            "BERT insertion time per vector too high");
        assert!(avg_search_time < Duration::from_millis(100),
            "BERT average search time too high");
        assert!(decay_time < Duration::from_millis(100),
            "BERT memory decay update time too high");
    }

    #[tokio::test]
    async fn test_temporal_features() {
        // Test parameters
        let vector_count = 50;
        let dim = 128;

        println!("\nRunning temporal features test with {} vectors of dimension {}", vector_count, dim);

        // Generate test data
        let test_vectors = generate_test_vectors(vector_count, dim);

        // Initialize storage
        let config = MemoryConfig {
            max_dimensions: dim,
            max_memories: vector_count * 2,
            temporal_weight: 0.5, // Equal weight for temporal and similarity
            ..MemoryConfig::default()
        };
        let metric = Arc::new(CosineDistance::new());
        let mut storage = MemoryStorage::new(config, metric);

        // Insert vectors with different timestamps and importance
        let now = SystemTime::now();

        // Insert vectors with varying timestamps and importance
        for (i, vec) in test_vectors.iter().enumerate() {
            let vector = Vector::new(
                format!("temporal_vector_{}", i),
                vec.clone(),
            );

            // Vary importance and timestamp based on index
            let importance = if i % 3 == 0 {
                0.9 // High importance
            } else if i % 3 == 1 {
                0.5 // Medium importance
            } else {
                0.2 // Low importance
            };

            // Vary age: recent, medium, old
            let timestamp = if i % 3 == 0 {
                // Recent - within last hour
                now - Duration::from_secs(i as u64 * 60) // i minutes ago
            } else if i % 3 == 1 {
                // Medium - within last day
                now - Duration::from_secs(i as u64 * 3600) // i hours ago
            } else {
                // Old - within last week
                now - Duration::from_secs(i as u64 * 3600 * 24) // i days ago
            };

            let temporal = TemporalVector::new(
                vector,
                MemoryAttributes {
                    timestamp,
                    importance,
                    context: format!("context_{}", i % 5), // 5 different contexts
                    decay_rate: 0.1,
                    relationships: Vec::new(),
                    access_count: i % 10, // Vary access count
                    last_access: timestamp,
                },
            );

            storage.save_memory(temporal).await.expect("Failed to save memory");
        }

        // Test search with temporal ordering
        let query = &test_vectors[0];
        let (results, search_time) = measure_time_async(|| async {
            storage.search_similar(query, 10).await.expect("Search failed")
        }).await;

        println!("Temporal search time: {:?}, found {} results", search_time, results.len());

        // Verify temporal ordering influence
        if !results.is_empty() {
            println!("Top 5 results with temporal ordering:");
            for (i, (memory, score)) in results.iter().take(5).enumerate() {
                let age = now.duration_since(memory.attributes.timestamp)
                    .unwrap_or(Duration::from_secs(0));

                println!("  {}. ID: {}, Score: {:.4}, Importance: {:.2}, Age: {:?}, Access Count: {}",
                    i+1, memory.vector.id, score, memory.attributes.importance, age, memory.attributes.access_count);
            }
        }

        // Test memory decay
        let (_, decay_time) = measure_time_async(|| async {
            storage.update_memory_decay().await.expect("Failed to update decay")
        }).await;

        println!("Temporal decay update time: {:?}", decay_time);

        // Verify decay effect
        let (memories_before_decay, _) = measure_time_async(|| async {
            storage.list_memories().await.expect("Failed to list memories")
        }).await;

        // Apply multiple decay cycles
        for i in 1..=3 {
            let _ = storage.update_memory_decay().await.expect("Failed to update decay");
            println!("Applied decay cycle {}", i);
        }

        let (memories_after_decay, _) = measure_time_async(|| async {
            storage.list_memories().await.expect("Failed to list memories")
        }).await;

        // Compare importance before and after decay
        println!("Importance changes after decay:");
        for (i, (before, after)) in memories_before_decay.iter().zip(memories_after_decay.iter()).take(5).enumerate() {
            println!("  {}. ID: {}, Before: {:.4}, After: {:.4}, Difference: {:.4}",
                i+1, before.vector.id, before.attributes.importance, after.attributes.importance,
                before.attributes.importance - after.attributes.importance);
        }
    }
}
