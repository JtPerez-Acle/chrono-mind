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
use crate::{RUNTIME, common::{config, generate_random_vectors, generate_timestamps}};

pub fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");
    
    // Setup memory storage with real-world configuration
    let mut store = RUNTIME.block_on(async {
        let metric = Arc::new(CosineDistance::new());
        MemoryStorage::new(MemoryConfig::default(), metric)
    });
    
    // Generate test data
    let test_vectors = generate_random_vectors(config::SMALL_DATASET, config::DIMS);
    let timestamps = generate_timestamps(config::SMALL_DATASET);
    
    // Initialize with test data
    RUNTIME.block_on(async {
        for (i, (vec, timestamp)) in test_vectors.iter().zip(timestamps.iter()).enumerate() {
            let importance = config::IMPORTANCE_RANGES[i % config::IMPORTANCE_RANGES.len()];
            let vector = Vector::new(
                format!("test_{}", i),
                vec.clone(),
            );
            let attrs = MemoryAttributes {
                timestamp: *timestamp,
                importance,
                context: format!("context_{}", i % 10),
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: 0,
                last_access: std::time::SystemTime::now(),
            };
            let temporal = TemporalVector::new(vector, attrs);
            let _ = store.save_memory(temporal).await;
        }
    });
    
    // Benchmark memory save operations
    group.bench_function("save_memory", |b| {
        b.iter(|| {
            let vector = Vector::new(
                "bench_test".to_string(),
                generate_random_vectors(1, config::DIMS)[0].clone(),
            );
            let attrs = MemoryAttributes {
                timestamp: std::time::SystemTime::now(),
                importance: 1.0,
                context: "bench_context".to_string(),
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: 0,
                last_access: std::time::SystemTime::now(),
            };
            let temporal = TemporalVector::new(vector, attrs);
            RUNTIME.block_on(async {
                let _ = black_box(store.save_memory(temporal).await);
            });
        });
    });
    
    // Benchmark vector similarity search
    group.bench_function("search_similar", |b| {
        let query = generate_random_vectors(1, config::DIMS)[0].clone();
        b.iter(|| {
            RUNTIME.block_on(async {
                let _ = black_box(store.search_similar(&query, 10).await);
            });
        });
    });
    
    group.finish();
}
