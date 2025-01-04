use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::{
    sync::Arc,
    time::{SystemTime, Duration},
    arch::x86_64::*,
};
use vector_store::{
    memory::{
        temporal::{MemoryConfig, MemoryStorage},
        types::{MemoryAttributes, TemporalVector, Vector},
    },
    storage::{
        metrics::CosineDistance,
        hnsw::{HNSWConfig, TemporalHNSW},
    },
    utils::monitoring::PerformanceMonitor,
};
use tokio::runtime::Runtime;
use rand::{Rng, thread_rng};
use rand_distr::{Normal, Distribution};
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use futures::future::join_all;
use rayon::prelude::*;

mod config {
    pub const DIMS: usize = 1536;
    pub const BATCH_SIZES: [usize; 5] = [100, 500, 1000, 5000, 10000];
    pub const WARM_UP_TIME: Duration = Duration::from_secs(2);
    pub const MEASUREMENT_TIME: Duration = Duration::from_secs(30);
    pub const MIN_SAMPLE_SIZE: usize = 30;
    
    // SIMD configuration
    pub const SIMD_WIDTH: usize = 16;
    pub const AVX512_ALIGNMENT: usize = 64;
    
    // HNSW parameters
    pub const HNSW_PARAMS: HNSWParams = HNSWParams {
        ef_construction: 128,
        ef_search: 64,
        m: 16,
        ml: 16,
    };
    
    // Temporal parameters
    pub const TEMPORAL_PARAMS: TemporalParams = TemporalParams {
        decay_rate: 0.1,
        importance_weight: 0.3,
        time_weight: 0.4,
    };
}

#[derive(Debug, Clone)]
struct HNSWParams {
    ef_construction: usize,
    ef_search: usize,
    m: usize,
    ml: usize,
}

#[derive(Debug, Clone)]
struct TemporalParams {
    decay_rate: f32,
    importance_weight: f32,
    time_weight: f32,
}

#[derive(Default)]
struct BenchMetrics {
    throughput: f64,
    latency_p50: f64,
    latency_p95: f64,
    latency_p99: f64,
    memory_usage: usize,
    cpu_usage: f64,
    cache_misses: u64,
    branch_misses: u64,
    temporal_score_avg: f64,
    index_size: usize,
    simd_utilization: f64,
    memory_bandwidth: f64,
}

impl std::fmt::Display for BenchMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\
            📊 Performance Metrics:\n\
            ├─ Throughput: {:.2} ops/s\n\
            ├─ Latency (ms):\n\
            │  ├─ p50: {:.2}\n\
            │  ├─ p95: {:.2}\n\
            │  └─ p99: {:.2}\n\
            ├─ Memory: {:.2} MB\n\
            ├─ CPU: {:.1}%\n\
            ├─ Cache Stats:\n\
            │  ├─ Cache Misses: {}\n\
            │  └─ Branch Misses: {}\n\
            ├─ Temporal Score: {:.3}\n\
            ├─ SIMD Utilization: {:.2}%\n\
            ├─ Memory Bandwidth: {:.2} GB/s\n\
            └─ Index Size: {} vectors",
            self.throughput,
            self.latency_p50,
            self.latency_p95,
            self.latency_p99,
            self.memory_usage as f64 / 1_048_576.0,
            self.cpu_usage,
            self.cache_misses,
            self.branch_misses,
            self.temporal_score_avg,
            self.simd_utilization,
            self.memory_bandwidth,
            self.index_size
        )
    }
}

#[cfg(target_arch = "x86_64")]
mod vector_ops {
    use super::*;
    
    #[target_feature(enable = "avx512f")]
    unsafe fn create_realistic_vector(id: &str, dims: usize, context: &str, importance: f32) -> TemporalVector {
        let mut rng = thread_rng();
        let normal = Normal::new(0.0, 1.0).unwrap();
        let mut data = Vec::with_capacity(dims);
        
        // Generate random values using AVX-512
        for _ in (0..dims).step_by(16) {
            let simd_vec = _mm512_set_ps(
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
                normal.sample(&mut rng) as f32,
            );
            
            let mut aligned_buf = vec![0_f32; 16];
            _mm512_store_ps(aligned_buf.as_mut_ptr(), simd_vec);
            data.extend_from_slice(&aligned_buf);
        }
        
        // Normalize using SIMD
        let norm = unsafe { simd_l2_norm(&data) };
        data.iter_mut().for_each(|x| *x /= norm);
        
        TemporalVector {
            id: id.to_string(),
            vector: Vector::new(data),
            attributes: MemoryAttributes {
                context: context.to_string(),
                importance,
                created_at: SystemTime::now(),
                last_accessed: SystemTime::now(),
                access_count: 0,
            },
        }
    }
    
    #[target_feature(enable = "avx512f")]
    unsafe fn simd_l2_norm(data: &[f32]) -> f32 {
        let mut sum = _mm512_setzero_ps();
        
        for chunk in data.chunks_exact(16) {
            let v = _mm512_loadu_ps(chunk.as_ptr());
            sum = _mm512_fmadd_ps(v, v, sum);
        }
        
        _mm512_reduce_add_ps(sum).sqrt()
    }
}

/// Thread-safe benchmark state for both MemoryStorage and HNSW
struct BenchState {
    memory_storage: Arc<RwLock<MemoryStorage>>,
    hnsw_index: Arc<RwLock<TemporalHNSW>>,
    runtime: Runtime,
    monitor: PerformanceMonitor,
}

impl BenchState {
    fn new(size: usize) -> Self {
        let config = MemoryConfig::default();
        let metric = Arc::new(CosineDistance::new());
        
        let memory_storage = MemoryStorage::new(metric.clone(), config.clone()).unwrap();
        
        let hnsw_config = HNSWConfig {
            ef_construction: config::HNSW_PARAMS.ef_construction,
            ef_search: config::HNSW_PARAMS.ef_search,
            m: config::HNSW_PARAMS.m,
            ml: config::HNSW_PARAMS.ml,
            ..Default::default()
        };
        
        let hnsw_index = TemporalHNSW::new(hnsw_config, metric);

        BenchState {
            memory_storage: Arc::new(RwLock::new(memory_storage)),
            hnsw_index: Arc::new(RwLock::new(hnsw_index)),
            runtime: Runtime::new().unwrap(),
            monitor: PerformanceMonitor::new(),
        }
    }

    fn batch_insert(&self, size: usize) {
        let vectors: Vec<_> = (0..size)
            .into_par_iter()
            .map(|i| unsafe {
                vector_ops::create_realistic_vector(
                    &format!("v{}", i),
                    config::DIMS,
                    "benchmark",
                    thread_rng().gen_range(0.0..1.0),
                )
            })
            .collect();

        // Insert into both storages
        let memory_storage = self.memory_storage.clone();
        let hnsw_index = self.hnsw_index.clone();
        
        self.runtime.block_on(async {
            let mut tasks = Vec::new();
            
            for v in vectors {
                let ms = memory_storage.clone();
                let hi = hnsw_index.clone();
                
                tasks.push(tokio::spawn(async move {
                    ms.write().insert_memory(v.clone()).unwrap();
                    hi.write().insert(&v).unwrap();
                }));
            }
            
            join_all(tasks).await
        });
    }

    async fn concurrent_search(&self, queries: &[Vec<f32>], k: usize) {
        let memory_storage = self.memory_storage.clone();
        let hnsw_index = self.hnsw_index.clone();
        
        let mut tasks = Vec::new();
        
        for query in queries {
            let ms = memory_storage.clone();
            let hi = hnsw_index.clone();
            let q = query.clone();
            
            tasks.push(tokio::spawn(async move {
                let _ = ms.read().search_similar(&q, k).await.unwrap();
                let _ = hi.read().search(&q, k).unwrap();
            }));
        }
        
        join_all(tasks).await;
    }

    fn update_decay(&self) {
        let duration = Duration::from_secs(3600); // 1 hour decay
        self.runtime.block_on(async {
            self.memory_storage.write().apply_decay(duration).unwrap();
        });
    }

    fn temporal_operations(&self, contexts: &[String]) {
        self.runtime.block_on(async {
            let storage = self.memory_storage.clone();
            
            // Test context-based operations
            for ctx in contexts {
                let _ = storage.read().search_by_context(ctx, &vec![0.0; config::DIMS], 10).await.unwrap();
                let _ = storage.read().compress_memories(ctx).unwrap();
            }
            
            // Test temporal decay and consolidation
            storage.write().apply_decay(Duration::from_secs(3600)).unwrap();
            storage.write().consolidate_memories().unwrap();
        });
    }
}

mod benchmarks {
    use super::*;
    
    pub fn bench_memory_operations(c: &mut Criterion) {
        let mut group = c.benchmark_group("Memory Operations");
        group.warm_up_time(config::WARM_UP_TIME);
        group.measurement_time(config::MEASUREMENT_TIME);
        group.sample_size(config::MIN_SAMPLE_SIZE);
        
        for &size in &config::BATCH_SIZES {
            group.bench_with_input(BenchmarkId::new("Batch Insert", size), &size, |b, &s| {
                let state = get_bench_state(s);
                b.iter(|| state.batch_insert(s));
            });
            
            group.bench_with_input(BenchmarkId::new("Concurrent Search", size), &size, |b, &s| {
                let state = get_bench_state(s);
                let queries = generate_query_vectors(s);
                b.iter(|| state.concurrent_search(&queries, 10));
            });
        }
        
        group.finish();
    }
    
    pub fn bench_temporal_features(c: &mut Criterion) {
        let mut group = c.benchmark_group("Temporal Features");
        group.warm_up_time(config::WARM_UP_TIME);
        group.measurement_time(config::MEASUREMENT_TIME);
        group.sample_size(config::MIN_SAMPLE_SIZE);
        
        let contexts = vec!["context1".to_string(), "context2".to_string()];
        
        for &size in &config::BATCH_SIZES {
            group.bench_with_input(BenchmarkId::new("Temporal Decay", size), &size, |b, &s| {
                let state = get_bench_state(s);
                b.iter(|| state.update_decay());
            });
            
            group.bench_with_input(BenchmarkId::new("Context Operations", size), &size, |b, &s| {
                let state = get_bench_state(s);
                b.iter(|| state.temporal_operations(&contexts));
            });
        }
        
        group.finish();
    }
    
    pub fn bench_hnsw_features(c: &mut Criterion) {
        let mut group = c.benchmark_group("HNSW Features");
        group.warm_up_time(config::WARM_UP_TIME);
        group.measurement_time(config::MEASUREMENT_TIME);
        group.sample_size(config::MIN_SAMPLE_SIZE);
        
        for &size in &config::BATCH_SIZES {
            group.bench_with_input(BenchmarkId::new("Graph Construction", size), &size, |b, &s| {
                let state = get_bench_state(s);
                b.iter(|| state.build_hnsw_graph());
            });
            
            group.bench_with_input(BenchmarkId::new("Similarity Search", size), &size, |b, &s| {
                let state = get_bench_state(s);
                let query = generate_query_vectors(1)[0].clone();
                b.iter(|| state.hnsw_search(&query, 10));
            });
        }
        
        group.finish();
    }
}

static BENCH_STATE: OnceCell<Mutex<Vec<(usize, Arc<BenchState>)>>> = OnceCell::new();

fn get_bench_state(size: usize) -> Arc<BenchState> {
    let states = BENCH_STATE.get_or_init(|| Mutex::new(Vec::new()));
    let mut states = states.lock();
    
    if let Some(state) = states.iter().find(|(s, _)| *s == size) {
        state.1.clone()
    } else {
        let state = Arc::new(BenchState::new(size));
        states.push((size, state.clone()));
        state
    }
}

fn generate_query_vectors(size: usize) -> Vec<Vec<f32>> {
    (0..size)
        .into_par_iter()
        .map(|_| {
            let mut rng = thread_rng();
            let normal = Normal::new(0.0, 1.0).unwrap();
            let mut data = Vec::with_capacity(config::DIMS);
            
            for _ in 0..config::DIMS {
                data.push(normal.sample(&mut rng) as f32);
            }
            
            data
        })
        .collect()
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .with_plots()
        .sample_size(config::MIN_SAMPLE_SIZE);
    targets = 
        benchmarks::bench_memory_operations,
        benchmarks::bench_temporal_features,
        benchmarks::bench_hnsw_features,
);
criterion_main!(benches);
