use criterion::{Criterion, measurement::WallTime};
use rand::Rng;
use std::time::{SystemTime, Duration};

pub mod config {
    use std::time::Duration;
    
    // Vector dimensions based on common embedding models
    pub const DIMS: usize = 768;  // BERT base embeddings
    pub const DIMS_SMALL: usize = 384;  // MiniLM embeddings
    pub const DIMS_LARGE: usize = 1024;  // BERT large embeddings
    
    // Dataset sizes for different scales
    pub const SMALL_DATASET: usize = 10_000;
    pub const MEDIUM_DATASET: usize = 100_000;
    pub const LARGE_DATASET: usize = 1_000_000;
    
    // HNSW parameters (optimized for accuracy/performance trade-off)
    pub const MAX_CONNECTIONS: usize = 64;
    pub const EF_CONSTRUCTION: usize = 200;
    pub const EF_SEARCH: usize = 100;
    
    // Temporal parameters
    pub const TIME_RANGES: [Duration; 4] = [
        Duration::from_secs(3600),      // 1 hour
        Duration::from_secs(86400),     // 1 day
        Duration::from_secs(604800),    // 1 week
        Duration::from_secs(2592000),   // 30 days
    ];
    
    pub const IMPORTANCE_RANGES: [f32; 3] = [0.3, 0.6, 1.0];
    
    // Benchmark configuration
    pub const WARM_UP_TIME: Duration = Duration::from_secs(5);
    pub const MEASUREMENT_TIME: Duration = Duration::from_secs(30);
    pub const MIN_SAMPLE_SIZE: usize = 50;
    
    // Target metrics
    pub const TARGET_RECALL: f32 = 0.95;
    pub const TARGET_QPS: usize = 1000;
    pub const MAX_LATENCY_MS: f32 = 1.0;
}

/// Generate random vectors for benchmarking
/// Returns normalized vectors to ensure consistent distance measurements
pub fn generate_random_vectors(count: usize, dims: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            let mut vec = (0..dims)
                .map(|_| rng.gen_range(-1.0..1.0))
                .collect::<Vec<f32>>();
            
            // Normalize the vector
            let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            vec.iter_mut().for_each(|x| *x /= magnitude);
            vec
        })
        .collect()
}

/// Generate timestamps distributed across different time ranges
pub fn generate_timestamps(count: usize) -> Vec<SystemTime> {
    let mut rng = rand::thread_rng();
    let now = SystemTime::now();
    
    (0..count)
        .map(|_| {
            let age = rng.gen_range(0..config::TIME_RANGES[3].as_secs());
            now - Duration::from_secs(age)
        })
        .collect()
}

/// Setup a benchmark group with standardized configuration
pub fn setup_benchmark_group<'a>(c: &'a mut Criterion, name: &str) -> criterion::BenchmarkGroup<'a, WallTime> {
    let mut group = c.benchmark_group(name);
    group.sample_size(config::MIN_SAMPLE_SIZE);
    group.measurement_time(config::MEASUREMENT_TIME);
    group.warm_up_time(config::WARM_UP_TIME);
    group
}
