use criterion::{black_box, Criterion};
use std::sync::Arc;
use vector_store::{
    storage::{
        hnsw::{HNSWConfig, TemporalHNSW},
        metrics::CosineDistance,
    },
    memory::types::{Vector, TemporalVector, MemoryAttributes},
};
use crate::{RUNTIME, common::{config, generate_realistic_embeddings, generate_temporal_patterns, QueryPattern}};

/// HNSW configurations based on production use cases
const HNSW_CONFIGS: [(usize, usize, usize); 3] = [
    // M, ef_construction, ef_search - tuned for different scenarios
    (16, 100, 50),   // Fast search (high QPS)
    (32, 200, 100),  // Balanced (default)
    (48, 400, 200),  // High accuracy
];

/// Helper function to normalize vector
fn normalize_vector(vec: &[f32]) -> Vec<f32> {
    let magnitude = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        vec.iter().map(|x| x / magnitude).collect()
    } else {
        vec.to_vec()
    }
}

/// Benchmark HNSW index operations
pub fn bench_hnsw_operations(c: &mut Criterion) {
    // Test different embedding models
    let models = ["bert-base", "ada-002", "e5-large", "minilm"];
    
    for model in models {
        let mut group = c.benchmark_group(format!("hnsw_ops_{}", model));
        
        // Test different HNSW configurations
        for &(m, ef_construction, _ef_search) in HNSW_CONFIGS.iter() {
            let config_name = match (m, ef_construction) {
                (16, 100) => "fast",
                (32, 200) => "balanced",
                (48, 400) => "accurate",
                _ => "custom",
            };
            
            // Initialize HNSW index
            let metric = Arc::new(CosineDistance::new());
            let config = HNSWConfig {
                max_dimensions: 128,
                max_connections: 16,       // Increased for better recall
                ef_construction: 200,      // Higher ef for better quality
                ef_search: 50,            // Balance between speed and recall
                temporal_weight: 0.5,     // Balance between recency and relevance
            };
            
            // Test with different dataset sizes
            let datasets = [
                ("small", config::DATASET_SMALL),
                ("medium", config::DATASET_MEDIUM),
                ("large", config::DATASET_LARGE),
            ];
            
            for (size, count) in datasets {
                // Generate realistic test data
                let test_vectors = generate_realistic_embeddings(count, model);
                let timestamps = generate_temporal_patterns(count, "mixed");
                
                // Benchmark index construction
                let bench_name = format!("build_{}_{}", size, config_name);
                group.bench_function(&bench_name, |b| {
                    b.iter(|| {
                        let hnsw = Arc::new(TemporalHNSW::new(config.clone(), metric.clone()));
                        
                        RUNTIME.block_on(async {
                            for (i, (vec, _timestamp)) in test_vectors.iter().zip(timestamps.iter()).enumerate() {
                                let (importance, _) = config::IMPORTANCE_CONFIGS[i % config::IMPORTANCE_CONFIGS.len()];
                                
                                let vector = Vector::new(
                                    format!("{}_{}_{}_{}", model, size, config_name, i),
                                    normalize_vector(vec), // Normalize the vector before insertion
                                );
                                
                                let attrs = MemoryAttributes {
                                    timestamp: std::time::SystemTime::now(),
                                    importance,
                                    context: format!("context_{}", i % 10),
                                    decay_rate: 0.1,
                                    relationships: Vec::new(),
                                    access_count: 0,
                                    last_access: std::time::SystemTime::now(),
                                };
                                
                                let temporal = TemporalVector::new(vector, attrs);
                                let _ = hnsw.insert(&temporal).await.expect("Failed to insert vector");
                            }
                        });
                        
                        black_box(hnsw)
                    });
                });
                
                // Create index for search benchmarks
                let mut hnsw = Arc::new(TemporalHNSW::new(config.clone(), metric.clone()));
                let config = HNSWConfig {
                    max_dimensions: test_vectors[0].len(),
                    max_connections: 16,       // Increased for better recall
                    ef_construction: 200,      // Higher ef for better quality
                    ef_search: 50,            // Balance between speed and recall
                    temporal_weight: 0.5,     // Balance between recency and relevance
                };
                RUNTIME.block_on(async {
                    for (i, (vec, _timestamp)) in test_vectors.iter().zip(timestamps.iter()).enumerate() {
                        let (importance, _) = config::IMPORTANCE_CONFIGS[i % config::IMPORTANCE_CONFIGS.len()];
                        
                        let vector = Vector::new(
                            format!("vector_{}", i),  // Unique ID for each vector
                            normalize_vector(vec), // Normalize the vector before insertion
                        );
                        
                        let temporal = TemporalVector::new(
                            vector,
                            MemoryAttributes {
                                timestamp: std::time::SystemTime::now(),
                                importance,
                                context: format!("context_{}", i % 10),
                                decay_rate: 0.1,
                                relationships: Vec::new(),
                                access_count: 0,
                                last_access: std::time::SystemTime::now(),
                            },
                        );
                        
                        hnsw.insert(&temporal).await.expect("Failed to insert vector");
                    }
                });
                
                // Benchmark search with different patterns
                for query_pattern in &[QueryPattern::ExactMatch, QueryPattern::Semantic, QueryPattern::Hybrid] {
                    let bench_name = format!("search_{}_{}_{:?}", size, config_name, query_pattern);
                    group.bench_function(&bench_name, |b| {
                        // Generate query based on pattern
                        let query = match query_pattern {
                            QueryPattern::ExactMatch => normalize_vector(&test_vectors[0]), // Normalize the query vector
                            QueryPattern::Semantic => {
                                let mut q = test_vectors[0].clone();
                                for x in q.iter_mut() {
                                    *x += rand::random::<f32>() * 0.2;
                                }
                                normalize_vector(&q)
                            },
                            QueryPattern::Hybrid => {
                                let mut q = test_vectors[0].clone();
                                for x in q.iter_mut() {
                                    *x += rand::random::<f32>() * 0.1;
                                }
                                normalize_vector(&q)
                            },
                        };
                        
                        b.iter(|| {
                            RUNTIME.block_on(async {
                                let k = match query_pattern {
                                    QueryPattern::ExactMatch => 1,     // Single best match
                                    QueryPattern::Semantic => 10,      // Multiple similar results
                                    QueryPattern::Hybrid => 5,         // Balance precision/recall
                                };
                                
                                let results = black_box(hnsw.search(&query, k).await.expect("Search failed"));
                                assert!(!results.is_empty(), "Search returned no results");
                                
                                // Validate search quality
                                match query_pattern {
                                    QueryPattern::ExactMatch => {
                                        // Should find exact match with high similarity
                                        assert!(results[0].1 >= 0.0, "Distance should be non-negative");
                                        assert!(results[0].1 < 0.1, "Distance too high for exact match");
                                    },
                                    QueryPattern::Semantic => {
                                        // Should find semantically similar results
                                        assert!(results.iter().all(|(_, d)| *d >= 0.0), "Distances should be non-negative");
                                        assert!(results[0].1 < 0.5, "Top result distance too high for semantic match");
                                    },
                                    QueryPattern::Hybrid => {
                                        // Mix of close and similar results
                                        assert!(results.iter().all(|(_, d)| *d >= 0.0), "Distances should be non-negative");
                                        assert!(results[0].1 < 0.3, "Top result distance too high for hybrid match");
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
