use criterion::{black_box, Criterion};
use std::sync::Arc;
use vector_store::{
    storage::{
        hnsw::{HNSWConfig, TemporalHNSW},
        metrics::CosineDistance,
    },
    memory::types::{Vector, TemporalVector, MemoryAttributes},
};
use crate::{RUNTIME, common::{config, generate_random_vectors, generate_timestamps}};

pub fn bench_hnsw_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_operations");
    
    // Setup HNSW index with real-world parameters
    let config = HNSWConfig {
        ef_construction: config::EF_CONSTRUCTION,
        max_connections: config::MAX_CONNECTIONS,
        ..Default::default()
    };
    
    let metric = Arc::new(CosineDistance::new());
    let hnsw = Arc::new(TemporalHNSW::new(config, metric));
    
    // Generate test data with realistic dimensions
    let test_vectors = generate_random_vectors(config::SMALL_DATASET, config::DIMS);
    let timestamps = generate_timestamps(config::SMALL_DATASET);
    
    // Create temporal vectors with varying importance
    let temporal_vectors: Vec<_> = test_vectors.into_iter()
        .zip(timestamps)
        .enumerate()
        .map(|(i, (vec, timestamp))| {
            let importance = config::IMPORTANCE_RANGES[i % config::IMPORTANCE_RANGES.len()];
            let vector = Vector::new(
                format!("test_{}", i),
                vec,
            );
            let attrs = MemoryAttributes {
                timestamp,
                importance,
                context: format!("context_{}", i % 10), // 10 different contexts
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: 0,
                last_access: std::time::SystemTime::now(),
            };
            TemporalVector::new(vector, attrs)
        })
        .collect();
    
    // Initialize with test data
    RUNTIME.block_on(async {
        for vector in &temporal_vectors {
            let _ = hnsw.insert(vector).await;
        }
    });
    
    // Benchmark insert with realistic vectors
    group.bench_function("insert", |b| {
        let hnsw = Arc::clone(&hnsw);
        b.iter(|| {
            let vector = TemporalVector::new(
                Vector::new(
                    "bench_test".to_string(),
                    generate_random_vectors(1, config::DIMS)[0].clone(),
                ),
                MemoryAttributes {
                    timestamp: std::time::SystemTime::now(),
                    importance: 1.0,
                    context: "bench_context".to_string(),
                    decay_rate: 0.1,
                    relationships: Vec::new(),
                    access_count: 0,
                    last_access: std::time::SystemTime::now(),
                },
            );
            
            RUNTIME.block_on(async {
                let _ = black_box(hnsw.insert(&vector).await);
            });
        });
    });
    
    // Benchmark search with realistic query vectors
    group.bench_function("search", |b| {
        let hnsw = Arc::clone(&hnsw);
        let query = generate_random_vectors(1, config::DIMS)[0].clone();
        
        b.iter(|| {
            RUNTIME.block_on(async {
                let _ = black_box(hnsw.search(&query, 10).await);
            });
        });
    });
    
    group.finish();
}
