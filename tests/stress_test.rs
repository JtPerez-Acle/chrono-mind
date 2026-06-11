//! Concurrency stress gates for the lock-free index and store
//! (`docs/DESIGN.md` §5 exit criteria).
//!
//! 16 threads, 100k+ mixed operations, then a full invariant sweep and a
//! recall check against brute force over the surviving vectors. Loom
//! verifies the primitives exhaustively at tiny scale; this verifies the
//! composed system at realistic scale.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, VectorIndex};
use chronomind::metric::{CosineDistance, DistanceMetric};
use chronomind::{ChronoMind, Config, Memory, MemoryAttributes, Vector};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const DIM: usize = 32;
const THREADS: usize = 16;
const OPS_PER_THREAD: usize = 6_500; // 16 * 6500 = 104k total ops
const SEED_VECTORS: usize = 1_000;

fn unit_vector(rng: &mut StdRng, dim: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// 16 threads hammer the index with ~75% searches, ~20% inserts, ~5%
/// removes of the thread's own earlier inserts. Afterwards: structural
/// invariants hold, tombstones never surface, and recall@10 over the
/// survivors beats 0.90.
#[test]
fn lockfree_index_survives_16_thread_hammering() {
    let index = Arc::new(LockFreeHnsw::with_seed(
        IndexParams::default(),
        Arc::new(CosineDistance::new()),
        0xA11CE,
    ));

    // Seed corpus, inserted single-threaded; handles 0..SEED_VECTORS.
    let mut rng = StdRng::seed_from_u64(0xA11CE);
    let seed_data: Vec<Vec<f32>> = (0..SEED_VECTORS)
        .map(|_| unit_vector(&mut rng, DIM))
        .collect();
    for v in &seed_data {
        index.insert(v);
    }

    let searches_done = Arc::new(AtomicUsize::new(0));

    // Each thread records what it inserted (handle -> vector) and which of
    // those it removed, so ground truth needs no cross-thread coordination.
    type ThreadLog = (Vec<(u32, Vec<f32>)>, Vec<u32>);
    let per_thread_log: Vec<ThreadLog> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let index = Arc::clone(&index);
                let searches_done = Arc::clone(&searches_done);
                s.spawn(move || {
                    let mut rng = StdRng::seed_from_u64(0xF00D + t as u64);
                    let mut inserted: Vec<(u32, Vec<f32>)> = Vec::new();
                    let mut removed: Vec<u32> = Vec::new();

                    for op in 0..OPS_PER_THREAD {
                        match op % 20 {
                            // 4/20 = 20% inserts
                            0 | 5 | 10 | 15 => {
                                let v = unit_vector(&mut rng, DIM);
                                let handle = index.insert(&v);
                                inserted.push((handle, v));
                            }
                            // 1/20 = 5% removes (own inserts only)
                            7 => {
                                if let Some((handle, _)) = inserted.get(removed.len()).cloned() {
                                    assert!(
                                        index.remove(handle),
                                        "thread {t}: remove of own handle {handle} failed"
                                    );
                                    removed.push(handle);
                                }
                            }
                            // 15/20 = 75% searches
                            _ => {
                                let q = unit_vector(&mut rng, DIM);
                                let results = index.search(&q, 50);
                                assert!(
                                    results.windows(2).all(|w| w[0].1 <= w[1].1),
                                    "thread {t}: unsorted results"
                                );
                                let mut ids: Vec<u32> = results.iter().map(|(id, _)| *id).collect();
                                ids.sort_unstable();
                                ids.dedup();
                                assert_eq!(
                                    ids.len(),
                                    results.len(),
                                    "thread {t}: duplicate ids in results"
                                );
                                searches_done.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    (inserted, removed)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    // Gate 1: structural invariants over the whole concurrently built graph.
    index.check_invariants().expect("graph invariants violated");

    // Reconstruct ground truth: seed vectors plus every surviving insert.
    let mut live: Vec<(u32, Vec<f32>)> = seed_data
        .iter()
        .enumerate()
        .map(|(i, v)| (i as u32, v.clone()))
        .collect();
    let mut all_removed: Vec<u32> = Vec::new();
    for (inserted, removed) in &per_thread_log {
        let removed_set: std::collections::HashSet<u32> = removed.iter().copied().collect();
        all_removed.extend(removed);
        live.extend(
            inserted
                .iter()
                .filter(|(h, _)| !removed_set.contains(h))
                .cloned(),
        );
    }
    assert_eq!(index.len(), live.len(), "live count drifted");

    // Gate 2: recall@10 >= 0.90 against brute force over survivors, and no
    // tombstone ever surfaces.
    let removed_set: std::collections::HashSet<u32> = all_removed.into_iter().collect();
    let metric = CosineDistance::new();
    let mut total_recall = 0.0;
    let queries = 20;
    for _ in 0..queries {
        let q = unit_vector(&mut rng, DIM);

        let mut expected: Vec<(f32, u32)> = live
            .iter()
            .map(|(id, v)| (metric.distance(v, &q), *id))
            .collect();
        expected.sort_by(|a, b| a.0.total_cmp(&b.0));
        let expected: Vec<u32> = expected.into_iter().take(10).map(|(_, id)| id).collect();

        let got: Vec<u32> = index
            .search(&q, 100)
            .into_iter()
            .take(10)
            .map(|(id, _)| id)
            .collect();
        for id in &got {
            assert!(!removed_set.contains(id), "tombstone {id} surfaced");
        }
        let hits = expected.iter().filter(|e| got.contains(e)).count();
        total_recall += hits as f64 / 10.0;
    }
    let recall = total_recall / queries as f64;
    assert!(
        recall >= 0.90,
        "post-stress recall@10 = {recall:.3} below the 0.90 gate \
         ({} live vectors, {} searches ran)",
        live.len(),
        searches_done.load(Ordering::Relaxed)
    );
}

/// Recall under live mutation: queries race concurrent inserts and must
/// still find the pre-churn ground truth.
///
/// The publish protocol guarantees readers see only fully formed graph
/// states; this gate checks the stronger property that mid-churn search
/// *quality* holds — a reader observing the graph between a writer's
/// backlink CASes still reaches the true neighbors. Ground truth is the
/// brute-force top-10 over the pre-churn corpus; results are filtered to
/// pre-churn handles (churn vectors may legitimately be nearer) and the
/// filtered recall must stay ≥ 0.90 while writers are actively inserting.
#[test]
fn recall_holds_during_concurrent_churn() {
    const BASE: usize = 2_000;
    const CHURN_PER_THREAD: usize = 1_500;
    const WRITERS: usize = 4;
    const K: usize = 10;

    let index = Arc::new(LockFreeHnsw::with_seed(
        IndexParams::default(),
        Arc::new(CosineDistance::new()),
        0xC4A2,
    ));

    let mut rng = StdRng::seed_from_u64(0xC4A2);
    let base: Vec<Vec<f32>> = (0..BASE).map(|_| unit_vector(&mut rng, DIM)).collect();
    for v in &base {
        index.insert(v); // handles 0..BASE, single-threaded
    }

    // Ground truth against the quiescent pre-churn corpus.
    let metric = CosineDistance::new();
    let queries: Vec<Vec<f32>> = (0..40).map(|_| unit_vector(&mut rng, DIM)).collect();
    let ground_truth: Vec<Vec<u32>> = queries
        .iter()
        .map(|q| {
            let mut scored: Vec<(f32, u32)> = base
                .iter()
                .enumerate()
                .map(|(i, v)| (metric.distance(v, q), i as u32))
                .collect();
            scored.sort_by(|a, b| a.0.total_cmp(&b.0));
            scored.into_iter().take(K).map(|(_, id)| id).collect()
        })
        .collect();

    let done = Arc::new(AtomicUsize::new(0));
    let (during_recall, during_queries) = std::thread::scope(|s| {
        for t in 0..WRITERS {
            let index = Arc::clone(&index);
            let done = Arc::clone(&done);
            s.spawn(move || {
                let mut rng = StdRng::seed_from_u64(0xC4A2 + 1 + t as u64);
                for _ in 0..CHURN_PER_THREAD {
                    index.insert(&unit_vector(&mut rng, DIM));
                }
                done.fetch_add(1, Ordering::Release);
            });
        }

        // Query continuously while writers are live; only rounds that ran
        // with at least one writer still active count toward the gate.
        let mut total = 0.0;
        let mut rounds = 0usize;
        while done.load(Ordering::Acquire) < WRITERS {
            for (q, truth) in queries.iter().zip(&ground_truth) {
                let got: Vec<u32> = index
                    .search(q, 100)
                    .into_iter()
                    .filter(|&(id, _)| (id as usize) < BASE) // pre-churn only
                    .take(K)
                    .map(|(id, _)| id)
                    .collect();
                let hits = truth.iter().filter(|e| got.contains(e)).count();
                total += hits as f64 / K as f64;
                rounds += 1;
            }
        }
        (total / rounds.max(1) as f64, rounds)
    });

    assert!(
        during_queries >= 40,
        "churn finished before queries could race it ({during_queries} rounds); \
         raise CHURN_PER_THREAD"
    );
    assert!(
        during_recall >= 0.90,
        "mid-churn recall@{K} = {during_recall:.3} below the 0.90 gate \
         ({during_queries} racing queries)"
    );

    // And the graph survived the churn structurally.
    index
        .check_invariants()
        .expect("post-churn invariants violated");
}

/// The full store under concurrent mixed load: inserts, searches, access
/// bumps, decay sweeps, and removes from 16 threads at once, all through
/// the public `&self` API.
#[test]
fn store_supports_fully_concurrent_use() {
    let store = Arc::new(
        ChronoMind::new(Config {
            dimensions: DIM,
            max_memories: 1_000_000,
            ..Config::default()
        })
        .unwrap(),
    );

    std::thread::scope(|s| {
        for t in 0..THREADS {
            let store = Arc::clone(&store);
            s.spawn(move || {
                let mut rng = StdRng::seed_from_u64(0xCAFE + t as u64);
                for i in 0..500 {
                    let id = format!("t{t}-m{i}");
                    store
                        .insert(Memory::new(
                            Vector::new(id.clone(), unit_vector(&mut rng, DIM)),
                            MemoryAttributes {
                                context: format!("ctx{}", t % 4),
                                ..MemoryAttributes::default()
                            },
                        ))
                        .unwrap();

                    match i % 5 {
                        0 => {
                            let results = store.search(&unit_vector(&mut rng, DIM), 5).unwrap();
                            assert!(results.len() <= 5);
                        }
                        1 => {
                            assert!(store.access(&id).is_some());
                        }
                        2 => {
                            store.apply_decay();
                        }
                        3 if i > 10 => {
                            store.remove(&format!("t{t}-m{}", i - 10));
                        }
                        _ => {
                            assert!(store.get(&id).is_some());
                        }
                    }
                }
            });
        }
    });

    // Every thread inserted 500 and removed ~98 of its own.
    let len = store.len();
    assert!(
        len > THREADS * 350 && len <= THREADS * 500,
        "unexpected store size {len}"
    );
    // The store still answers correctly after the storm.
    let probe = store.get("t0-m499").unwrap();
    assert_eq!(probe.vector.id, "t0-m499");
    let stats = store.stats();
    assert_eq!(stats.total_memories, len);
}
