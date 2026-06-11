//! Recall gates: both HNSW indexes must agree with brute force.
//!
//! These are the Phase 2 exit criteria from `docs/DESIGN.md` §4, run
//! against the locked baseline AND the lock-free index — the latter must
//! pass the exact gates that validate the former. Fully deterministic:
//! seeded RNG, fixed dataset sizes, single-threaded insertion.

use std::sync::Arc;

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, RwLockHnsw, VectorIndex};
use chronomind::metric::{CosineDistance, DistanceMetric};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const N: usize = 2_000;
const QUERIES: usize = 20;
const K: usize = 10;
const GATE: f64 = 0.95;

#[derive(Clone, Copy, Debug)]
enum Impl {
    Baseline,
    LockFree,
}

fn build_index(which: Impl, seed: u64) -> Box<dyn VectorIndex> {
    let params = IndexParams::default();
    let metric = Arc::new(CosineDistance::new());
    match which {
        Impl::Baseline => Box::new(RwLockHnsw::with_seed(params, metric, seed)),
        Impl::LockFree => Box::new(LockFreeHnsw::with_seed(params, metric, seed)),
    }
}

/// Random unit vector, dimension `dim`.
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

/// A 768-dimensional vector confined to a random low-dimensional subspace —
/// the shape real embedding data takes. Uniform high-dim noise is covered
/// separately by the connectivity gate.
fn embedded_vector(rng: &mut StdRng, basis: &[Vec<f32>], dim: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; dim];
    for b in basis {
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
}

/// Exact top-`k` ids by metric distance.
fn brute_force_top_k(data: &[Vec<f32>], query: &[f32], k: usize) -> Vec<u32> {
    let metric = CosineDistance::new();
    let mut scored: Vec<(f32, u32)> = data
        .iter()
        .enumerate()
        .map(|(i, v)| (metric.distance(v, query), i as u32))
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    scored.into_iter().take(k).map(|(_, i)| i).collect()
}

/// Mean recall@K of an index against brute force.
///
/// Index handles are assigned densely in insertion order, so handle `i`
/// corresponds to `data[i]`; brute force uses the same indexing.
fn measure_recall(
    which: Impl,
    data: &[Vec<f32>],
    queries: &[Vec<f32>],
    ef: usize,
    seed: u64,
) -> f64 {
    let index = build_index(which, seed);
    for v in data {
        index.insert(v).unwrap();
    }

    let mut total = 0.0;
    for query in queries {
        let expected = brute_force_top_k(data, query, K);
        let got: Vec<u32> = index
            .search(query, K.max(ef))
            .into_iter()
            .take(K)
            .map(|(id, _)| id)
            .collect();
        let hits = expected.iter().filter(|e| got.contains(e)).count();
        total += hits as f64 / K as f64;
    }
    total / queries.len() as f64
}

fn uniform_dataset(dim: usize, seed: u64) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let data = (0..N).map(|_| unit_vector(&mut rng, dim)).collect();
    let queries = (0..QUERIES).map(|_| unit_vector(&mut rng, dim)).collect();
    (data, queries)
}

fn embedding_dataset(seed: u64) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    const DIM: usize = 768;
    const INTRINSIC: usize = 16;
    let mut rng = StdRng::seed_from_u64(seed);
    let basis: Vec<Vec<f32>> = (0..INTRINSIC).map(|_| unit_vector(&mut rng, DIM)).collect();
    let data = (0..N)
        .map(|_| embedded_vector(&mut rng, &basis, DIM))
        .collect();
    let queries = (0..QUERIES)
        .map(|_| embedded_vector(&mut rng, &basis, DIM))
        .collect();
    (data, queries)
}

fn assert_gate(which: Impl, recall: f64, gate: f64, label: &str) {
    assert!(
        recall >= gate,
        "{which:?}: recall@{K} = {recall:.3} below the {gate} gate ({label}, N {N})"
    );
}

fn gate_dim_32(which: Impl) {
    let (data, queries) = uniform_dataset(32, 0xC0FFEE);
    let ef = IndexParams::default().ef_search;
    let recall = measure_recall(which, &data, &queries, ef, 0xC0FFEE);
    assert_gate(which, recall, GATE, "dim 32 uniform, default ef");
}

fn gate_dim_768_embedding(which: Impl) {
    let (data, queries) = embedding_dataset(0xBEEF);
    let ef = IndexParams::default().ef_search;
    let recall = measure_recall(which, &data, &queries, ef, 0xBEEF);
    assert_gate(which, recall, GATE, "dim 768 embedding-like, default ef");
}

/// Connectivity gate: even on uniform 768-d data — where distance
/// concentration blinds greedy navigation at low ef — a healthy graph must
/// reach the true neighbors once ef gives it room. A recall collapse here
/// means broken graph construction, not hard data.
fn gate_dim_768_uniform_high_ef(which: Impl) {
    let (data, queries) = uniform_dataset(768, 0xBEEF);
    let recall = measure_recall(which, &data, &queries, 200, 0xBEEF);
    assert_gate(which, recall, GATE, "dim 768 uniform, ef 200");
}

/// Delete a third of the dataset, then verify search agrees with brute
/// force over the survivors and never leaks a tombstone.
fn gate_tombstones(which: Impl) {
    let dim = 32;
    let mut rng = StdRng::seed_from_u64(0xDEAD);
    let data: Vec<Vec<f32>> = (0..N).map(|_| unit_vector(&mut rng, dim)).collect();

    let index = build_index(which, 0xDEAD);
    for v in &data {
        index.insert(v).unwrap();
    }
    for id in 0..(N as u32) {
        if id % 3 == 0 {
            assert!(index.remove(id));
        }
    }

    let survivors: Vec<(u32, &Vec<f32>)> = data
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 3 != 0)
        .map(|(i, v)| (i as u32, v))
        .collect();

    let metric = CosineDistance::new();
    let mut total = 0.0;
    for _ in 0..QUERIES {
        let query = unit_vector(&mut rng, dim);

        let mut expected: Vec<(f32, u32)> = survivors
            .iter()
            .map(|(id, v)| (metric.distance(v, &query), *id))
            .collect();
        expected.sort_by(|a, b| a.0.total_cmp(&b.0));
        let expected: Vec<u32> = expected.into_iter().take(K).map(|(_, id)| id).collect();

        let got: Vec<u32> = index
            .search(&query, 50)
            .into_iter()
            .take(K)
            .map(|(id, _)| id)
            .collect();
        assert!(
            got.iter().all(|id| id % 3 != 0),
            "{which:?}: tombstoned id leaked into results"
        );
        let hits = expected.iter().filter(|e| got.contains(e)).count();
        total += hits as f64 / K as f64;
    }
    assert_gate(which, total / QUERIES as f64, 0.90, "post-delete, dim 32");
}

mod baseline {
    use super::*;

    #[test]
    fn recall_at_10_gate_dim_32() {
        gate_dim_32(Impl::Baseline);
    }

    #[test]
    fn recall_at_10_gate_dim_768_embedding_like() {
        gate_dim_768_embedding(Impl::Baseline);
    }

    #[test]
    fn recall_at_10_gate_dim_768_uniform_high_ef() {
        gate_dim_768_uniform_high_ef(Impl::Baseline);
    }

    #[test]
    fn recall_survives_tombstones() {
        gate_tombstones(Impl::Baseline);
    }
}

mod lockfree {
    use super::*;

    #[test]
    fn recall_at_10_gate_dim_32() {
        gate_dim_32(Impl::LockFree);
    }

    #[test]
    fn recall_at_10_gate_dim_768_embedding_like() {
        gate_dim_768_embedding(Impl::LockFree);
    }

    #[test]
    fn recall_at_10_gate_dim_768_uniform_high_ef() {
        gate_dim_768_uniform_high_ef(Impl::LockFree);
    }

    #[test]
    fn recall_survives_tombstones() {
        gate_tombstones(Impl::LockFree);
    }
}

/// Diagnostic, not a gate: print the ef/recall curve for uniform 768-d data.
#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn diagnose_uniform_768_ef_curve() {
    for ef in [50usize, 100, 200, 500, 1000] {
        let (data, queries) = uniform_dataset(768, 0xBEEF);
        for which in [Impl::Baseline, Impl::LockFree] {
            let recall = measure_recall(which, &data, &queries, ef, 0xBEEF);
            println!("{which:?}: ef = {ef:4}: recall@{K} = {recall:.3}");
        }
    }
}
