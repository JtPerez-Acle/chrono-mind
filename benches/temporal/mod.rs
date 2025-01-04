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
use crate::RUNTIME;

pub fn bench_temporal_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_operations");
    
    // Setup
    let mut store = RUNTIME.block_on(async {
        let metric = Arc::new(CosineDistance::new());
        MemoryStorage::new(MemoryConfig::default(), metric)
    });
    
    // Setup test data
    let test_vectors = (0..100).map(|i| {
        let vector = Vector::new(
            format!("test_{}", i),
            vec![0.1, 0.2, 0.3],
        );
        let attrs = MemoryAttributes {
            timestamp: std::time::SystemTime::now(),
            importance: 1.0,
            context: "test_context".to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: std::time::SystemTime::now(),
        };
        TemporalVector::new(vector, attrs)
    }).collect::<Vec<_>>();
    
    // Initialize with test data
    RUNTIME.block_on(async {
        for vector in &test_vectors {
            let _ = store.save_memory(vector.clone()).await;
        }
    });
    
    // Benchmark get_related_memories
    group.bench_function("get_related_memories", |b| {
        b.iter(|| {
            RUNTIME.block_on(async {
                let _ = black_box(store.get_related_memories("test_0", 2).await);
            });
        });
    });
    
    // Benchmark search_by_context
    group.bench_function("search_by_context", |b| {
        b.iter(|| {
            let query = vec![0.1, 0.2, 0.3];
            RUNTIME.block_on(async {
                let _ = black_box(store.search_by_context("test_context", &query, 10).await);
            });
        });
    });
    
    group.finish();
}
