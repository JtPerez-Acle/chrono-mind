//! Reads under concurrent writes — the scenario ChronoMind is built for.
//!
//! Run with:
//! ```text
//! cargo bench --bench concurrency --features bench-external
//! ```
//!
//! Every other benchmark here is single-threaded or write-only, which is the
//! one framing where a lock-free index has nothing to prove. This one
//! measures what actually motivated the design: **does read throughput (and
//! tail latency) survive while writers are hammering the same index?**
//!
//! For each system we measure read QPS and read latency twice — once with
//! readers alone (uncontended), once with readers AND writers running
//! together (contended) — and report the retention (contended / uncontended).
//! A wait-free reader should retain ~100%; a design where writers exclude
//! readers should collapse.
//!
//! Contenders:
//! - `chronomind`   lock-free: wait-free reads, lock-free writes
//! - `sharded16`    16 RwLock shards (what you'd deploy to scale a locked design)
//! - `rwlock`       one RwLock over the whole graph (writers exclude readers)
//! - `usearch`      C++ engine with thread-safe add + search

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, RwLockHnsw, ShardedRwLockHnsw, VectorIndex};
use chronomind::metric::CosineDistance;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const DIM: usize = 256;
const INTRINSIC: usize = 24;
const BASE: usize = 20_000; // vectors present before the measurement
const EXTRA: usize = 40_000; // pool writers insert from (cycled)
const QUERIES: usize = 1000;
const READERS: usize = 4;
const WRITERS: usize = 4;
const EF: usize = 50;
const K: usize = 10;
const DUR: Duration = Duration::from_secs(3);
const SEED: u64 = 0xC0FFEE;

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
                let c: f32 = rng.gen_range(-1.0..1.0);
                for (o, x) in v.iter_mut().zip(b) {
                    *o += c * x;
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

struct Phase {
    read_qps: f64,
    p99_us: f64,
    writes: u64,
}

/// Run `readers` reader threads (and `writers` writer threads) for `DUR`,
/// timing every search. Returns read QPS and read-latency percentiles.
fn run_phase(
    readers: usize,
    writers: usize,
    search: &(impl Fn(usize) + Sync),
    insert: &(impl Fn(usize) -> bool + Sync),
) -> Phase {
    let deadline = Instant::now() + DUR;
    let mut lats: Vec<u64> = Vec::new();
    let mut writes: u64 = 0;
    std::thread::scope(|s| {
        let rhandles: Vec<_> = (0..readers)
            .map(|t| {
                s.spawn(move || {
                    let mut lat: Vec<u64> = Vec::with_capacity(1 << 16);
                    let mut i = t;
                    while Instant::now() < deadline {
                        let st = Instant::now();
                        search(i);
                        lat.push(st.elapsed().as_nanos() as u64);
                        i += readers;
                    }
                    lat
                })
            })
            .collect();
        let whandles: Vec<_> = (0..writers)
            .map(|t| {
                s.spawn(move || {
                    let mut n: u64 = 0;
                    let mut i = t;
                    while Instant::now() < deadline {
                        if insert(i) {
                            n += 1;
                        }
                        i += writers;
                    }
                    n
                })
            })
            .collect();
        for h in rhandles {
            lats.extend(h.join().unwrap());
        }
        for h in whandles {
            writes += h.join().unwrap();
        }
    });
    lats.sort_unstable();
    let n = lats.len();
    let pct = |q: f64| {
        if n == 0 {
            0.0
        } else {
            lats[((n as f64 * q) as usize).min(n - 1)] as f64 / 1000.0
        }
    };
    Phase {
        read_qps: n as f64 / DUR.as_secs_f64(),
        p99_us: pct(0.99),
        writes,
    }
}

struct Row {
    name: &'static str,
    build_s: f64,
    uncontended: Phase,
    contended: Phase,
}

fn print_table(rows: &[Row]) {
    println!();
    println!(
        "reads under writes: dim={DIM}, base={BASE}, {READERS} readers, \
         {WRITERS} writers, ef={EF}, {}s/phase",
        DUR.as_secs()
    );
    println!();
    println!("| system | build (s) | read QPS (idle) | read QPS (under writes) | retention | p99 idle (us) | p99 under writes (us) | writes done |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");
    for r in rows {
        let ret = if r.uncontended.read_qps > 0.0 {
            100.0 * r.contended.read_qps / r.uncontended.read_qps
        } else {
            0.0
        };
        println!(
            "| {} | {:.1} | {:.0} | {:.0} | {:.0}% | {:.1} | {:.1} | {} |",
            r.name,
            r.build_s,
            r.uncontended.read_qps,
            r.contended.read_qps,
            ret,
            r.uncontended.p99_us,
            r.contended.p99_us,
            r.contended.writes,
        );
    }
    println!();
}

fn params() -> IndexParams {
    IndexParams {
        max_connections: 16,
        ef_construction: 100,
        ef_search: EF,
    }
}

fn bench_chrono_family(
    name: &'static str,
    index: Arc<dyn VectorIndex>,
    base: &[Vec<f32>],
    extra: &[Vec<f32>],
    queries: &[Vec<f32>],
) -> Row {
    let t = Instant::now();
    for v in base {
        index.insert(v);
    }
    let build_s = t.elapsed().as_secs_f64();

    let search = |i: usize| {
        std::hint::black_box(index.search(&queries[i % QUERIES], EF));
    };
    let insert = |i: usize| -> bool { index.insert(&extra[i % EXTRA]).is_some() };

    let uncontended = run_phase(READERS, 0, &search, &insert);
    let contended = run_phase(READERS, WRITERS, &search, &insert);
    Row {
        name,
        build_s,
        uncontended,
        contended,
    }
}

fn bench_usearch(base: &[Vec<f32>], extra: &[Vec<f32>], queries: &[Vec<f32>]) -> Row {
    use usearch::{new_index, IndexOptions, MetricKind, ScalarKind};

    let options = IndexOptions {
        dimensions: DIM,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        connectivity: 16,
        expansion_add: 100,
        expansion_search: EF,
        ..Default::default()
    };
    let index = new_index(&options).expect("usearch index");
    // Headroom for the base set plus everything writers can insert.
    index.reserve(BASE + 1_000_000).expect("reserve");

    let t = Instant::now();
    for (i, v) in base.iter().enumerate() {
        index.add(i as u64, v).expect("add");
    }
    let build_s = t.elapsed().as_secs_f64();

    let next_id = AtomicU64::new(base.len() as u64);
    let search = |i: usize| {
        std::hint::black_box(index.search(&queries[i % QUERIES], K).expect("search"));
    };
    let insert = |i: usize| -> bool {
        let id = next_id.fetch_add(1, Ordering::Relaxed);
        index.add(id, &extra[i % EXTRA]).is_ok()
    };

    let uncontended = run_phase(READERS, 0, &search, &insert);
    let contended = run_phase(READERS, WRITERS, &search, &insert);
    Row {
        name: "usearch (C++)",
        build_s,
        uncontended,
        contended,
    }
}

fn main() {
    println!("building datasets...");
    let base = embedding_dataset(BASE, SEED);
    let extra = embedding_dataset(EXTRA, SEED ^ 0xAAAA);
    let queries = embedding_dataset(QUERIES, SEED ^ 0xFACE);

    let metric = Arc::new(CosineDistance::new());
    let rows = vec![
        bench_chrono_family(
            "chronomind (lock-free)",
            Arc::new(LockFreeHnsw::with_seed(params(), metric.clone(), SEED)),
            &base,
            &extra,
            &queries,
        ),
        bench_chrono_family(
            "sharded16",
            Arc::new(ShardedRwLockHnsw::with_seed(params(), metric.clone(), SEED)),
            &base,
            &extra,
            &queries,
        ),
        bench_chrono_family(
            "rwlock (one lock)",
            Arc::new(RwLockHnsw::with_seed(params(), metric.clone(), SEED)),
            &base,
            &extra,
            &queries,
        ),
        bench_usearch(&base, &extra, &queries),
    ];
    print_table(&rows);
}
