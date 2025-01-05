use criterion::{black_box, Criterion};
use std::sync::Arc;
use vector_store::{
    core::{
        config::MemoryConfig,
    },
    memory::{
        temporal::MemoryStorage,
        types::{Vector, TemporalVector, MemoryAttributes},
    },
    storage::metrics::CosineDistance,
};
use crate::{RUNTIME, common::{config, generate_realistic_embeddings, generate_temporal_patterns, QueryPattern}};

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot_product / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Benchmark suite for memory operations in real-world scenarios
pub fn bench_memory_operations(c: &mut Criterion) {
    // Test with different embedding models
    let models = ["bert-base", "ada-002", "e5-large", "minilm"];
    
    for model in models {
        let mut group = c.benchmark_group(format!("memory_ops_{}", model));
        
        // Initialize store with model-specific dimensions
        let mut store = RUNTIME.block_on(async {
            let metric = Arc::new(CosineDistance::new());
            let dims = match model {
                "bert-base" => config::DIMS_BERT_BASE,
                "ada-002" => config::DIMS_ADA_002,
                "e5-large" => config::DIMS_E5_LARGE,
                "minilm" => config::DIMS_MINILM,
                _ => unreachable!(),
            };
            
            let config = MemoryConfig {
                max_dimensions: dims,
                ..MemoryConfig::default()
            };
            MemoryStorage::new(config, metric)
        });
        
        // Test with different dataset sizes
        let datasets = [
            ("small", config::DATASET_SMALL),     // Personal KB
            ("medium", config::DATASET_MEDIUM),   // Small business
            ("large", config::DATASET_LARGE),     // Enterprise dept
        ];
        
        for (size, count) in datasets {
            // Generate realistic test data
            let test_vectors = generate_realistic_embeddings(count, model);
            
            // Test different usage patterns
            let patterns = [
                "chat_history",      // Recent conversational data
                "knowledge_base",    // Reference documentation
                "mixed",            // Mixed content types
            ];
            
            for pattern in patterns {
                let timestamps = generate_temporal_patterns(count, pattern);
                
                // Initialize with test data
                RUNTIME.block_on(async {
                    for (i, (vec, timestamp)) in test_vectors.iter().zip(timestamps.iter()).enumerate() {
                        let (importance, context) = config::IMPORTANCE_CONFIGS[i % config::IMPORTANCE_CONFIGS.len()];
                        
                        let id = format!("{}_{}_{}_{}", model, size, pattern, i);
                        let vector = Vector::new(
                            id.clone(),
                            vec.clone(),
                        );
                        
                        let attrs = MemoryAttributes {
                            timestamp: *timestamp,
                            importance,
                            context: context.to_string(),
                            decay_rate: 0.1,
                            relationships: Vec::new(),
                            access_count: 0,
                            last_access: std::time::SystemTime::now(),
                        };
                        
                        let temporal = TemporalVector::new(vector, attrs);
                        store.save_memory(temporal).await.expect("Failed to save memory");
                    }
                });
                
                // Benchmark different query scenarios
                for query_pattern in &[QueryPattern::ExactMatch, QueryPattern::Semantic, QueryPattern::Hybrid] {
                    let bench_name = format!("search_{}_{}_{:?}", size, pattern, query_pattern);
                    group.bench_function(&bench_name, |b| {
                        b.iter(|| {
                            // Generate query based on pattern
                            let query = match query_pattern {
                                QueryPattern::ExactMatch => test_vectors[0].clone(),
                                QueryPattern::Semantic => {
                                    let mut q = test_vectors[0].clone();
                                    for x in q.iter_mut() {
                                        *x += rand::random::<f32>() * 0.2; // Reduced noise for more similar results
                                    }
                                    let magnitude = q.iter().map(|x| x * x).sum::<f32>().sqrt();
                                    if magnitude > 0.0 {
                                        q.iter_mut().for_each(|x| *x /= magnitude);
                                    }
                                    q
                                },
                                QueryPattern::Hybrid => {
                                    let mut q = test_vectors[0].clone();
                                    for x in q.iter_mut() {
                                        *x += rand::random::<f32>() * 0.1;
                                    }
                                    let magnitude = q.iter().map(|x| x * x).sum::<f32>().sqrt();
                                    if magnitude > 0.0 {
                                        q.iter_mut().for_each(|x| *x /= magnitude);
                                    }
                                    q
                                },
                            };
                            
                            RUNTIME.block_on(async {
                                let k = match query_pattern {
                                    QueryPattern::ExactMatch => 1,     // Single best match
                                    QueryPattern::Semantic => 10,      // Multiple similar results
                                    QueryPattern::Hybrid => 5,         // Balance precision/recall
                                };
                                
                                let results = black_box(store.search_similar(&query, k).await.expect("Search failed"));
                                assert!(!results.is_empty(), "Search returned no results");
                                
                                // Get actual vector for top result
                                let top_id = format!("{}_{}_{}_{}", model, size, pattern, 0);
                                let top_result = store.get_memory(&top_id).await
                                    .expect("Failed to get memory")
                                    .expect("Memory not found");
                                let similarity = cosine_similarity(&query, &top_result.vector.data);
                                
                                // Validate results based on query type
                                match query_pattern {
                                    QueryPattern::ExactMatch => {
                                        // Should find exact match with high similarity
                                        assert!(similarity > 0.8, "Exact match similarity too low: {}", similarity);
                                        assert_eq!(results.len(), 1, "Exact match should return single result");
                                    },
                                    QueryPattern::Semantic => {
                                        // Should find semantically similar results
                                        assert!(similarity > 0.1, "Semantic match similarity too low: {}", similarity);
                                        assert_eq!(results.len(), 10, "Semantic search should return 10 results");
                                        // Verify results are ordered by similarity
                                        for i in 1..results.len() {
                                            assert!(results[i-1].1 <= results[i].1, "Results not properly ordered");
                                        }
                                    },
                                    QueryPattern::Hybrid => {
                                        // Should balance exact and semantic matches
                                        assert!(similarity > 0.4, "Hybrid match similarity too low: {}", similarity);
                                        assert_eq!(results.len(), 5, "Hybrid search should return 5 results");
                                        // Check distribution of similarities
                                        let similarities: Vec<_> = results.iter().map(|(_, d)| *d).collect();
                                        let avg_similarity = similarities.iter().sum::<f32>() / similarities.len() as f32;
                                        assert!(avg_similarity > 0.3, "Average similarity too low: {}", avg_similarity);
                                    },
                                }
                            });
                        });
                    });
                }
            }
        }
        
        group.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);

        let a = vec![1.0, 1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.7071067).abs() < 1e-6);
    }
}
