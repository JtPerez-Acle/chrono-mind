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

// Benchmark configuration
const DIMS: usize = 1536;
const BATCH_SIZES: [usize; 4] = [100, 500, 1000, 5000];
const WARM_UP_TIME: Duration = Duration::from_secs(2);
const MEASUREMENT_TIME: Duration = Duration::from_secs(30);
const MIN_SAMPLE_SIZE: usize = 20;
const SIMD_WIDTH: usize = 16;

// HNSW specific settings
const EF_CONSTRUCTION: usize = 128;
const EF_SEARCH: usize = 64;
const M: usize = 16;
const ML: usize = 16;

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
            self.index_size
        )
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn create_realistic_vector(id: &str, dims: usize, context: &str, importance: f32) -> TemporalVector {
    let mut rng = thread_rng();
    let normal = Normal::new(0.0, 1.0).unwrap();
    let mut data = Vec::with_capacity(dims);
    
    // Generate random values using AVX2
    for _ in (0..dims).step_by(8) {
        let simd_vec = _mm256_set_ps(
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
            normal.sample(&mut rng) as f32,
        );
        
        // Normalize using AVX2
        let sum_squares = _mm256_dp_ps(simd_vec, simd_vec, 0xFF);
        let norm = _mm256_sqrt_ps(sum_squares);
        let normalized = _mm256_div_ps(simd_vec, norm);
        
        let mut temp = [0.0f32; 8];
        _mm256_storeu_ps(temp.as_mut_ptr(), normalized);
        data.extend_from_slice(&temp);
    }

    // Create temporal attributes
    let attrs = MemoryAttributes {
        context: context.to_string(),
        importance,
        timestamp: SystemTime::now(),
        decay_rate: 0.1,
    };

    TemporalVector {
        id: id.to_string(),
        data,
        attributes: attrs,
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
            ef_construction: EF_CONSTRUCTION,
            ef_search: EF_SEARCH,
            m: M,
            ml: ML,
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
                create_realistic_vector(
                    &format!("v{}", i),
                    DIMS,
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
                let _ = storage.read().search_by_context(ctx, &vec![0.0; DIMS], 10).await.unwrap();
                let _ = storage.read().compress_memories(ctx).unwrap();
            }
            
            // Test temporal decay and consolidation
            storage.write().apply_decay(Duration::from_secs(3600)).unwrap();
            storage.write().consolidate_memories().unwrap();
        });
    }
}

fn percentile(durations: &[Duration], p: f64) -> f64 {
    let mut sorted: Vec<_> = durations.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = (sorted.len() as f64 * p).round() as usize;
    sorted[pos.min(sorted.len() - 1)]
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

fn bench_memory_batch_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Batch Insertion");
    
    for size in BATCH_SIZES.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let state = get_bench_state(size);
            b.iter(|| state.batch_insert(size));
        });
    }
    
    group.finish();
}

fn bench_memory_concurrent_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("Concurrent Search");
    let runtime = Runtime::new().unwrap();
    
    for size in BATCH_SIZES.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let state = get_bench_state(size);
            let queries: Vec<_> = (0..10)
                .map(|_| unsafe { create_realistic_vector("q", DIMS, "query", 1.0) })
                .map(|v| v.data)
                .collect();
                
            b.to_async(&runtime).iter(|| state.concurrent_search(&queries, 10));
        });
    }
    
    group.finish();
}

fn bench_temporal_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("Temporal Operations");
    
    for size in BATCH_SIZES.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let state = get_bench_state(size);
            let contexts = vec!["ctx1".to_string(), "ctx2".to_string(), "ctx3".to_string()];
            
            b.iter(|| state.temporal_operations(&contexts));
        });
    }
    
    group.finish();
}

fn bench_hnsw_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Operations");
    let runtime = Runtime::new().unwrap();
    
    for size in BATCH_SIZES.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let state = get_bench_state(size);
            let vector = unsafe { create_realistic_vector("test", DIMS, "benchmark", 1.0) };
            
            b.iter(|| {
                state.runtime.block_on(async {
                    state.hnsw_index.write().insert(&vector).unwrap();
                    state.hnsw_index.read().search(&vector.data, 10).unwrap();
                });
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_memory_batch_insertion,
    bench_memory_concurrent_search,
    bench_temporal_operations,
    bench_hnsw_operations,
);

criterion_main!(benches);
