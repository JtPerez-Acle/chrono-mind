//! A/B benchmarks: lock-free HNSW vs the RwLock baseline.
//!
//! Run with `cargo bench`. Three workloads, each across 1/2/4/8 threads:
//!
//! - `insert_throughput`: concurrent index construction
//! - `search_qps`: pure queries against a pre-built index
//! - `mixed_90_10`: 90% searches / 10% inserts, the realistic
//!   agent-memory workload
//!
//! Data is embedding-like (768-d vectors on a random 16-d subspace, unit
//! norm) — the same distribution the recall gates use, modeling real
//! embeddings rather than uniform noise. Datasets are seeded and
//! deterministic.
//!
//! Sizes are chosen so the full suite finishes in minutes; the point is
//! the *relative* scaling of the two implementations under contention,
//! which is size-stable, not absolute big-corpus numbers.

use std::sync::Arc;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, RwLockHnsw, VectorIndex};
use chronomind::metric::CosineDistance;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const DIM: usize = 768;
const INTRINSIC: usize = 16;
const INSERT_N: usize = 4_000;
const PREBUILT_N: usize = 10_000;
const SEARCH_OPS: usize = 2_000;
const MIXED_OPS: usize = 2_000;
const THREAD_COUNTS: [usize; 4] = [1, 2, 4, 8];
const SEED: u64 = 0x5EED;

fn params() -> IndexParams {
    IndexParams {
        max_connections: 16,
        ef_construction: 100,
        ef_search: 50,
    }
}

#[derive(Clone, Copy)]
enum Impl {
    LockFree,
    Baseline,
}

impl Impl {
    fn name(self) -> &'static str {
        match self {
            Impl::LockFree => "lockfree",
            Impl::Baseline => "rwlock",
        }
    }

    fn build(self) -> Arc<dyn VectorIndex> {
        let metric = Arc::new(CosineDistance::new());
        match self {
            Impl::LockFree => Arc::new(LockFreeHnsw::with_seed(params(), metric, SEED)),
            Impl::Baseline => Arc::new(RwLockHnsw::with_seed(params(), metric, SEED)),
        }
    }
}

fn embedding_dataset(n: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let basis: Vec<Vec<f32>> = (0..INTRINSIC)
        .map(|_| {
            let mut v: Vec<f32> = (0..DIM).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            for x in &mut v {
                *x /= norm;
            }
            v
        })
        .collect();

    (0..n)
        .map(|_| {
            let mut v = vec![0.0f32; DIM];
            for b in &basis {
                let coeff: f32 = rng.gen_range(-1.0..1.0);
                for (out, x) in v.iter_mut().zip(b) {
                    *out += coeff * x;
                }
            }
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut v {
                    *x /= norm;
                }
            }
            v
        })
        .collect()
}

/// Run `op` for every item of `work`, split evenly across `threads`,
/// returning the wall time of the parallel section.
fn timed_parallel<T: Sync>(threads: usize, work: &[T], op: impl Fn(&T) + Sync) -> Duration {
    let chunk = work.len().div_ceil(threads);
    let op = &op;
    let start = Instant::now();
    std::thread::scope(|s| {
        for part in work.chunks(chunk.max(1)) {
            s.spawn(move || {
                for item in part {
                    op(item);
                }
            });
        }
    });
    start.elapsed()
}

fn insert_throughput(c: &mut Criterion) {
    let data = embedding_dataset(INSERT_N, SEED);
    let mut group = c.benchmark_group("insert_throughput");
    group.sample_size(10);
    group.throughput(criterion::Throughput::Elements(INSERT_N as u64));

    for which in [Impl::LockFree, Impl::Baseline] {
        for threads in THREAD_COUNTS {
            group.bench_with_input(
                BenchmarkId::new(which.name(), threads),
                &threads,
                |b, &threads| {
                    b.iter_custom(|iters| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iters {
                            let index = which.build();
                            total += timed_parallel(threads, &data, |v| {
                                index.insert(v);
                            });
                        }
                        total
                    });
                },
            );
        }
    }
    group.finish();
}

fn search_qps(c: &mut Criterion) {
    let corpus = embedding_dataset(PREBUILT_N, SEED);
    let queries = embedding_dataset(SEARCH_OPS, SEED ^ 0xFACE);

    let mut group = c.benchmark_group("search_qps");
    group.sample_size(10);
    group.throughput(criterion::Throughput::Elements(SEARCH_OPS as u64));

    for which in [Impl::LockFree, Impl::Baseline] {
        let index = which.build();
        for v in &corpus {
            index.insert(v);
        }
        for threads in THREAD_COUNTS {
            group.bench_with_input(
                BenchmarkId::new(which.name(), threads),
                &threads,
                |b, &threads| {
                    b.iter_custom(|iters| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iters {
                            total += timed_parallel(threads, &queries, |q| {
                                std::hint::black_box(index.search(q, 50));
                            });
                        }
                        total
                    });
                },
            );
        }
    }
    group.finish();
}

fn mixed_90_10(c: &mut Criterion) {
    let corpus = embedding_dataset(PREBUILT_N, SEED);
    // One op stream: every 10th op inserts a fresh vector, the rest search.
    let stream = embedding_dataset(MIXED_OPS, SEED ^ 0xD1CE);

    let mut group = c.benchmark_group("mixed_90_10");
    group.sample_size(10);
    group.throughput(criterion::Throughput::Elements(MIXED_OPS as u64));

    let indexed: Vec<(usize, &Vec<f32>)> = stream.iter().enumerate().collect();
    for which in [Impl::LockFree, Impl::Baseline] {
        for threads in THREAD_COUNTS {
            // One pre-built index per configuration. Inserted vectors
            // accumulate across that configuration's samples (bounded
            // drift: ~200 per iteration on a 10k corpus); rebuilding per
            // iteration would dominate the benchmark's wall time.
            let index = which.build();
            for v in &corpus {
                index.insert(v);
            }
            group.bench_with_input(
                BenchmarkId::new(which.name(), threads),
                &threads,
                |b, &threads| {
                    b.iter_custom(|iters| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iters {
                            total += timed_parallel(threads, &indexed, |&(i, v)| {
                                if i % 10 == 0 {
                                    index.insert(v);
                                } else {
                                    std::hint::black_box(index.search(v, 50));
                                }
                            });
                        }
                        total
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, insert_throughput, search_qps, mixed_90_10);
criterion_main!(benches);
