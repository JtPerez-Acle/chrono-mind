//! Head-to-head against external open-source ANN libraries.
//!
//! Run with:
//! ```text
//! cargo bench --bench external --features bench-external
//! ```
//!
//! Contenders, all at M=16 / efConstruction=100 / efSearch=50 (or each
//! library's nearest equivalent), all over the same seeded 768-d
//! embedding-like dataset (10k corpus, 100 queries):
//!
//! - `chronomind` — our lock-free index
//! - `chrono-sharded16` — our sharded-RwLock baseline
//! - `instant-distance` — pure-Rust HNSW (bulk build only, internally
//!   parallel; no incremental insert or delete)
//! - `hnsw_rs` — pure-Rust HNSW (rayon parallel insert)
//! - `usearch` — C++ engine with Rust bindings (SIMD-heavy, f32 forced
//!   for apples-to-apples; thread-safe adds)
//!
//! Measured: build wall time (single-thread and best-available parallel),
//! search QPS (1 and 8 threads), and recall@10 against brute-force cosine
//! ground truth. All vectors are unit-norm, so cosine and Euclidean
//! rankings coincide — each library runs its native metric.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, ShardedRwLockHnsw, VectorIndex};
use chronomind::metric::{CosineDistance, DistanceMetric};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const DIM: usize = 768;
const INTRINSIC: usize = 16;
const N: usize = 10_000;
const QUERIES: usize = 100;
const K: usize = 10;
const SEARCH_PASS: usize = 20; // each timed pass runs QUERIES * SEARCH_PASS searches
const SEED: u64 = 0x0BEC;

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

fn ground_truth(corpus: &[Vec<f32>], queries: &[Vec<f32>]) -> Vec<Vec<usize>> {
    let metric = CosineDistance::new();
    queries
        .iter()
        .map(|q| {
            let mut scored: Vec<(f32, usize)> = corpus
                .iter()
                .enumerate()
                .map(|(i, v)| (metric.distance(v, q), i))
                .collect();
            scored.sort_by(|a, b| a.0.total_cmp(&b.0));
            scored.into_iter().take(K).map(|(_, i)| i).collect()
        })
        .collect()
}

fn recall(expected: &[Vec<usize>], got: &[Vec<usize>]) -> f64 {
    let mut total = 0.0;
    for (want, have) in expected.iter().zip(got) {
        let hits = want.iter().filter(|w| have.contains(w)).count();
        total += hits as f64 / K as f64;
    }
    total / expected.len() as f64
}

/// Run `op(query_index)` for `total` iterations split across `threads`.
fn timed_parallel(threads: usize, total: usize, op: impl Fn(usize) + Sync) -> Duration {
    let op = &op;
    let per = total / threads;
    let start = Instant::now();
    std::thread::scope(|s| {
        for t in 0..threads {
            s.spawn(move || {
                for i in (t * per)..((t + 1) * per) {
                    op(i);
                }
            });
        }
    });
    start.elapsed()
}

/// Best of three timed passes, as ops/sec.
fn best_qps(threads: usize, total: usize, op: impl Fn(usize) + Sync) -> f64 {
    let mut best = Duration::MAX;
    for _ in 0..3 {
        best = best.min(timed_parallel(threads, total, &op));
    }
    total as f64 / best.as_secs_f64()
}

struct Row {
    name: &'static str,
    build_1t: Option<Duration>,
    build_mt: Option<(Duration, &'static str)>,
    qps_1t: f64,
    qps_8t: f64,
    recall_at_10: f64,
}

fn print_table(rows: &[Row]) {
    println!();
    println!("| system | build 1T | build parallel | search QPS 1T | search QPS 8T | recall@10 |");
    println!("|---|---:|---:|---:|---:|---:|");
    for r in rows {
        let b1 = r
            .build_1t
            .map(|d| format!("{:.2}s", d.as_secs_f64()))
            .unwrap_or_else(|| "—".into());
        let bm = r
            .build_mt
            .map(|(d, label)| format!("{:.2}s ({label})", d.as_secs_f64()))
            .unwrap_or_else(|| "—".into());
        println!(
            "| {} | {} | {} | {:.0} | {:.0} | {:.3} |",
            r.name, b1, bm, r.qps_1t, r.qps_8t, r.recall_at_10
        );
    }
    println!();
}

fn bench_chronomind(
    corpus: &[Vec<f32>],
    queries: &[Vec<f32>],
    truth: &[Vec<usize>],
    sharded: bool,
) -> Row {
    let params = IndexParams {
        max_connections: 16,
        ef_construction: 100,
        ef_search: 50,
    };
    let metric = Arc::new(CosineDistance::new());
    let build = |threads: usize| -> (Arc<dyn VectorIndex>, Duration) {
        let index: Arc<dyn VectorIndex> = if sharded {
            Arc::new(ShardedRwLockHnsw::with_seed(
                params.clone(),
                metric.clone(),
                SEED,
            ))
        } else {
            Arc::new(LockFreeHnsw::with_seed(
                params.clone(),
                metric.clone(),
                SEED,
            ))
        };
        let chunk = corpus.len().div_ceil(threads);
        let started = Instant::now();
        std::thread::scope(|s| {
            for part in corpus.chunks(chunk) {
                let index = Arc::clone(&index);
                s.spawn(move || {
                    for v in part {
                        index.insert(v).unwrap();
                    }
                });
            }
        });
        (index, started.elapsed())
    };

    let (_, build_1t) = build(1);
    let (index, build_mt) = build(8);

    // NOTE: parallel builds assign handles in nondeterministic order, so
    // recall is measured against a single-threaded build where handle i
    // equals corpus index i.
    let (st_index, _) = build(1);
    let got: Vec<Vec<usize>> = queries
        .iter()
        .map(|q| {
            st_index
                .search(q, 50)
                .into_iter()
                .take(K)
                .map(|(id, _)| id as usize)
                .collect()
        })
        .collect();

    let qps_1t = best_qps(1, QUERIES * SEARCH_PASS, |i| {
        std::hint::black_box(index.search(&queries[i % QUERIES], 50));
    });
    let qps_8t = best_qps(8, QUERIES * SEARCH_PASS, |i| {
        std::hint::black_box(index.search(&queries[i % QUERIES], 50));
    });

    Row {
        name: if sharded {
            "chrono-sharded16"
        } else {
            "chronomind (lock-free)"
        },
        build_1t: Some(build_1t),
        build_mt: Some((build_mt, "8T")),
        qps_1t,
        qps_8t,
        recall_at_10: recall(truth, &got),
    }
}

fn bench_instant_distance(corpus: &[Vec<f32>], queries: &[Vec<f32>], truth: &[Vec<usize>]) -> Row {
    use instant_distance::{Builder, Search};

    #[derive(Clone)]
    struct P(Vec<f32>);
    impl instant_distance::Point for P {
        fn distance(&self, other: &Self) -> f32 {
            // Euclidean on unit vectors: ranking-equivalent to cosine.
            self.0
                .iter()
                .zip(&other.0)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
                .sqrt()
        }
    }

    let points: Vec<P> = corpus.iter().map(|v| P(v.clone())).collect();
    let values: Vec<usize> = (0..corpus.len()).collect();

    // instant-distance only does bulk construction (internally parallel).
    let started = Instant::now();
    let map = Builder::default()
        .ef_construction(100)
        .ef_search(50)
        .seed(SEED)
        .build(points, values);
    let build_mt = started.elapsed();

    let got: Vec<Vec<usize>> = queries
        .iter()
        .map(|q| {
            let mut search = Search::default();
            map.search(&P(q.clone()), &mut search)
                .take(K)
                .map(|item| *item.value)
                .collect()
        })
        .collect();

    let qps_1t = best_qps(1, QUERIES * SEARCH_PASS, |i| {
        let mut search = Search::default();
        std::hint::black_box(
            map.search(&P(queries[i % QUERIES].clone()), &mut search)
                .take(K)
                .count(),
        );
    });
    let qps_8t = best_qps(8, QUERIES * SEARCH_PASS, |i| {
        let mut search = Search::default();
        std::hint::black_box(
            map.search(&P(queries[i % QUERIES].clone()), &mut search)
                .take(K)
                .count(),
        );
    });

    Row {
        name: "instant-distance",
        build_1t: None,
        build_mt: Some((build_mt, "internal")),
        qps_1t,
        qps_8t,
        recall_at_10: recall(truth, &got),
    }
}

fn bench_hnsw_rs(corpus: &[Vec<f32>], queries: &[Vec<f32>], truth: &[Vec<usize>]) -> Row {
    use hnsw_rs::prelude::*;

    let build_st = || {
        let hnsw = Hnsw::<f32, DistCosine>::new(16, N, 16, 100, DistCosine {});
        let started = Instant::now();
        for (i, v) in corpus.iter().enumerate() {
            hnsw.insert((v.as_slice(), i));
        }
        (hnsw, started.elapsed())
    };
    let (_, build_1t) = build_st();

    let hnsw_mt = Hnsw::<f32, DistCosine>::new(16, N, 16, 100, DistCosine {});
    let data: Vec<(&Vec<f32>, usize)> = corpus.iter().zip(0..).collect();
    let started = Instant::now();
    hnsw_mt.parallel_insert(&data);
    let build_mt = started.elapsed();

    // Recall from the deterministic single-threaded build.
    let (hnsw, _) = build_st();
    let got: Vec<Vec<usize>> = queries
        .iter()
        .map(|q| {
            hnsw.search(q.as_slice(), K, 50)
                .into_iter()
                .map(|n| n.d_id)
                .collect()
        })
        .collect();

    let qps_1t = best_qps(1, QUERIES * SEARCH_PASS, |i| {
        std::hint::black_box(hnsw.search(queries[i % QUERIES].as_slice(), K, 50));
    });
    let qps_8t = best_qps(8, QUERIES * SEARCH_PASS, |i| {
        std::hint::black_box(hnsw.search(queries[i % QUERIES].as_slice(), K, 50));
    });

    Row {
        name: "hnsw_rs",
        build_1t: Some(build_1t),
        build_mt: Some((build_mt, "rayon")),
        qps_1t,
        qps_8t,
        recall_at_10: recall(truth, &got),
    }
}

fn bench_usearch(corpus: &[Vec<f32>], queries: &[Vec<f32>], truth: &[Vec<usize>]) -> Row {
    use usearch::{new_index, IndexOptions, MetricKind, ScalarKind};

    let options = IndexOptions {
        dimensions: DIM,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32, // apples-to-apples: no quantization
        connectivity: 16,
        expansion_add: 100,
        expansion_search: 50,
        ..Default::default()
    };

    let build = |threads: usize| {
        let index = new_index(&options).expect("usearch index");
        index.reserve(N).expect("reserve");
        let chunk = corpus.len().div_ceil(threads);
        let started = Instant::now();
        std::thread::scope(|s| {
            for (t, part) in corpus.chunks(chunk).enumerate() {
                let index = &index;
                s.spawn(move || {
                    for (i, v) in part.iter().enumerate() {
                        index.add((t * chunk + i) as u64, v).expect("add");
                    }
                });
            }
        });
        (index, started.elapsed())
    };

    let (_, build_1t) = build(1);
    let (index, build_mt) = build(8);

    let got: Vec<Vec<usize>> = queries
        .iter()
        .map(|q| {
            let matches = index.search(q, K).expect("search");
            matches.keys.iter().map(|&k| k as usize).collect()
        })
        .collect();

    let qps_1t = best_qps(1, QUERIES * SEARCH_PASS, |i| {
        std::hint::black_box(index.search(&queries[i % QUERIES], K).expect("search"));
    });
    let qps_8t = best_qps(8, QUERIES * SEARCH_PASS, |i| {
        std::hint::black_box(index.search(&queries[i % QUERIES], K).expect("search"));
    });

    Row {
        name: "usearch (C++)",
        build_1t: Some(build_1t),
        build_mt: Some((build_mt, "8T")),
        qps_1t,
        qps_8t,
        recall_at_10: recall(truth, &got),
    }
}

fn main() {
    println!(
        "external comparison: N={N}, dim={DIM} (intrinsic {INTRINSIC}), \
         {QUERIES} queries, M=16, efC=100, efS=50, f32 everywhere"
    );
    let corpus = embedding_dataset(N, SEED);
    let queries = embedding_dataset(QUERIES, SEED ^ 0xFACE);
    let truth = ground_truth(&corpus, &queries);

    let rows = vec![
        bench_chronomind(&corpus, &queries, &truth, false),
        bench_chronomind(&corpus, &queries, &truth, true),
        bench_instant_distance(&corpus, &queries, &truth),
        bench_hnsw_rs(&corpus, &queries, &truth),
        bench_usearch(&corpus, &queries, &truth),
    ];
    print_table(&rows);
}
