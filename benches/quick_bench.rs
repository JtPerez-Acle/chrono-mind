use criterion::{criterion_group, criterion_main, Criterion, black_box};
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::time::SystemTime;
use vector_store::{
    core::config::MemoryConfig,
    memory::{
        temporal::MemoryStorage,
        types::{MemoryAttributes, TemporalVector, Vector},
    },
    storage::metrics::CosineDistance,
};

// Create a shared runtime for async operations
pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().unwrap()
});

// Helper function to run async code in benchmarks
pub fn run_async<F, R>(future: F) -> R
where
    F: std::future::Future<Output = R>,
{
    RUNTIME.block_on(future)
}

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

// Benchmark core operations
fn bench_core_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_operations");

    // Test with a small dataset (100 vectors)
    let vector_count = 100;
    let dim = 64; // Smaller dimension for quick testing

    // Generate test data
    let test_vectors = generate_test_vectors(vector_count, dim);

    // Benchmark memory storage initialization
    group.bench_function("init_memory_storage", |b| {
        b.iter(|| {
            let config = MemoryConfig {
                max_dimensions: dim,
                max_memories: vector_count,
                ..MemoryConfig::default()
            };
            let metric = Arc::new(CosineDistance::new());
            black_box(MemoryStorage::new(config, metric))
        });
    });

    // Benchmark vector insertion
    group.bench_function("insert_vectors", |b| {
        b.iter(|| {
            let config = MemoryConfig {
                max_dimensions: dim,
                max_memories: vector_count,
                ..MemoryConfig::default()
            };
            let metric = Arc::new(CosineDistance::new());
            let mut storage = MemoryStorage::new(config, metric);

            run_async(async {
                for (i, vec) in test_vectors.iter().take(10).enumerate() {
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
            });

            black_box(storage)
        });
    });

    // Benchmark vector search
    group.bench_function("search_vectors", |b| {
        // Setup storage with vectors
        let config = MemoryConfig {
            max_dimensions: dim,
            max_memories: vector_count,
            ..MemoryConfig::default()
        };
        let metric = Arc::new(CosineDistance::new());
        let mut storage = MemoryStorage::new(config, metric);

        run_async(async {
            for (i, vec) in test_vectors.iter().take(10).enumerate() {
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
        });

        // Benchmark search
        b.iter(|| {
            run_async(async {
                let query = &test_vectors[0];
                let results = storage.search_similar(query, 5).await.expect("Search failed");
                black_box(results)
            })
        });
    });

    // Benchmark memory decay
    group.bench_function("update_memory_decay", |b| {
        // Setup storage with vectors
        let config = MemoryConfig {
            max_dimensions: dim,
            max_memories: vector_count,
            ..MemoryConfig::default()
        };
        let metric = Arc::new(CosineDistance::new());
        let mut storage = MemoryStorage::new(config, metric);

        run_async(async {
            for (i, vec) in test_vectors.iter().take(10).enumerate() {
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
        });

        // Benchmark decay update
        b.iter(|| {
            run_async(async {
                black_box(storage.update_memory_decay().await.expect("Failed to update decay"));
            })
        });
    });

    group.finish();
}

criterion_group!(benches, bench_core_operations);
criterion_main!(benches);
