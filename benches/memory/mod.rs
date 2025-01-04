use criterion::{black_box, criterion_group, Criterion};
use chrono_mind::{
    core::config::MemoryConfig,
    memory::{types::{MemoryAttributes, TemporalVector, Vector}, traits::VectorStorage},
    storage::persistence::MemoryBackend,
};
use rand::prelude::*;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;

const MEMORY_DIMS: usize = 768;
const BATCH_SIZES: [usize; 4] = [1, 10, 100, 1000];

fn generate_memory(rng: &mut impl RngCore) -> TemporalVector {
    let vector = Vector::new(
        format!("mem_{}", rng.gen::<u64>()),
        (0..MEMORY_DIMS).map(|_| rng.gen()).collect(),
    );
    
    let attributes = MemoryAttributes {
        timestamp: SystemTime::now(),
        importance: rng.gen_range(0.0..1.0),
        context: format!("context_{}", rng.gen::<u8>()),
        decay_rate: rng.gen_range(0.1..0.5),
        relationships: vec![],
        access_count: 0,
        last_access: SystemTime::now(),
    };
    
    TemporalVector::new(vector, attributes)
}

pub fn bench_memory_operations(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let config = MemoryConfig::default();
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_operations");
    group.measurement_time(Duration::from_secs(10));
    
    // Benchmark insertion
    for &batch_size in &BATCH_SIZES {
        group.bench_with_input(format!("insert_{}", batch_size), &batch_size, |b, &size| {
            let memories: Vec<_> = (0..size)
                .map(|_| generate_memory(&mut rng))
                .collect();
            
            b.iter(|| {
                let backend = black_box(MemoryBackend::new(config.clone()));
                for memory in &memories {
                    rt.block_on(async {
                        backend.insert_memory(memory.clone()).await.unwrap();
                    });
                }
            });
        });
    }
    
    // Benchmark retrieval
    let backend = MemoryBackend::new(config.clone());
    let memory = generate_memory(&mut rng);
    let id = memory.vector.id.clone();
    rt.block_on(async {
        backend.insert_memory(memory.clone()).await.unwrap();
    });
    
    group.bench_function("get_memory", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(backend.get_memory(&id).await.unwrap());
            });
        });
    });
    
    // Benchmark search
    let backend = MemoryBackend::new(config.clone());
    for _ in 0..100 {
        rt.block_on(async {
            backend.insert_memory(generate_memory(&mut rng)).await.unwrap();
        });
    }
    
    group.bench_function("search_context", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(backend.search_by_context("context_1", 10).await.unwrap());
            });
        });
    });
    
    group.finish();
}

criterion_group! {
    name = memory;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(10));
    targets = bench_memory_operations
}
