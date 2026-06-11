//! Recall gates: the HNSW index must agree with brute force.
//!
//! These are the Phase 2 exit criteria from `docs/DESIGN.md` §4 and are the
//! regression harness the lock-free rewrite is verified against. Fully
//! deterministic: seeded RNG, fixed dataset sizes.

use std::sync::Arc;

use chronomind::config::IndexParams;
use chronomind::index::{RwLockHnsw, VectorIndex};
use chronomind::metric::{CosineDistance, DistanceMetric};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const N: usize = 2_000;
const QUERIES: usize = 20;
const K: usize = 10;
const GATE: f64 = 0.95;

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

/// Mean recall@K of the index against brute force over `QUERIES` queries.
fn measure_recall(dim: usize, seed: u64) -> f64 {
    let mut rng = StdRng::seed_from_u64(seed);
    let data: Vec<Vec<f32>> = (0..N).map(|_| unit_vector(&mut rng, dim)).collect();
    let queries: Vec<Vec<f32>> = (0..QUERIES).map(|_| unit_vector(&mut rng, dim)).collect();
    measure_recall_with(dim, data, queries, IndexParams::default().ef_search, seed)
}

fn measure_recall_with(
    _dim: usize,
    data: Vec<Vec<f32>>,
    queries: Vec<Vec<f32>>,
    ef: usize,
    seed: u64,
) -> f64 {
    let index = RwLockHnsw::with_seed(
        IndexParams::default(),
        Arc::new(CosineDistance::new()),
        seed,
    );
    for v in &data {
        // Index handles are assigned densely in insertion order, so handle
        // i corresponds to data[i]; brute force uses the same indexing.
        index.insert(v);
    }

    let mut total = 0.0;
    for query in &queries {
        let expected = brute_force_top_k(&data, query, K);
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

/// Diagnostic, not a gate: print the ef/recall curve for uniform 768-d data.
#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn diagnose_uniform_768_ef_curve() {
    for ef in [50usize, 100, 200, 500, 1000] {
        let mut rng = StdRng::seed_from_u64(0xBEEF);
        let data: Vec<Vec<f32>> = (0..N).map(|_| unit_vector(&mut rng, 768)).collect();
        let queries: Vec<Vec<f32>> = (0..QUERIES).map(|_| unit_vector(&mut rng, 768)).collect();
        let recall = measure_recall_with(768, data, queries, ef, 0xBEEF);
        println!("ef = {ef:4}: recall@{K} = {recall:.3}");
    }
}

/// A 768-dimensional vector confined to a random `intrinsic`-dimensional
/// subspace — the shape real embedding data takes (embeddings concentrate
/// on low-dimensional manifolds; uniform 768-d noise does not occur in
/// practice and is covered by the connectivity gate below).
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

#[test]
fn recall_at_10_gate_dim_32() {
    let recall = measure_recall(32, 0xC0FFEE);
    assert!(
        recall >= GATE,
        "recall@{K} = {recall:.3} below the {GATE} gate (dim 32, N {N})"
    );
}

#[test]
fn recall_at_10_gate_dim_768_embedding_like() {
    const DIM: usize = 768;
    const INTRINSIC: usize = 16;
    let mut rng = StdRng::seed_from_u64(0xBEEF);
    let basis: Vec<Vec<f32>> = (0..INTRINSIC).map(|_| unit_vector(&mut rng, DIM)).collect();
    let data: Vec<Vec<f32>> = (0..N)
        .map(|_| embedded_vector(&mut rng, &basis, DIM))
        .collect();
    let queries: Vec<Vec<f32>> = (0..QUERIES)
        .map(|_| embedded_vector(&mut rng, &basis, DIM))
        .collect();

    let recall = measure_recall_with(DIM, data, queries, IndexParams::default().ef_search, 0xBEEF);
    assert!(
        recall >= GATE,
        "recall@{K} = {recall:.3} below the {GATE} gate (dim 768 embedding-like, N {N})"
    );
}

/// Connectivity gate: even on uniform 768-d data — where distance
/// concentration blinds greedy navigation at low ef — a healthy graph must
/// reach the true neighbors once ef gives it room. A recall collapse here
/// means broken graph construction, not hard data.
#[test]
fn recall_at_10_gate_dim_768_uniform_high_ef() {
    let mut rng = StdRng::seed_from_u64(0xBEEF);
    let data: Vec<Vec<f32>> = (0..N).map(|_| unit_vector(&mut rng, 768)).collect();
    let queries: Vec<Vec<f32>> = (0..QUERIES).map(|_| unit_vector(&mut rng, 768)).collect();

    let recall = measure_recall_with(768, data, queries, 200, 0xBEEF);
    assert!(
        recall >= GATE,
        "recall@{K} = {recall:.3} below the {GATE} gate (dim 768 uniform, ef 200, N {N})"
    );
}

#[test]
fn recall_survives_tombstones() {
    // Delete a third of the dataset, then verify search still agrees with
    // brute force over the survivors.
    let dim = 32;
    let mut rng = StdRng::seed_from_u64(0xDEAD);
    let data: Vec<Vec<f32>> = (0..N).map(|_| unit_vector(&mut rng, dim)).collect();

    let index = RwLockHnsw::with_seed(
        IndexParams::default(),
        Arc::new(CosineDistance::new()),
        0xDEAD,
    );
    for v in &data {
        index.insert(v);
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
            "tombstoned id leaked into results"
        );
        let hits = expected.iter().filter(|e| got.contains(e)).count();
        total += hits as f64 / K as f64;
    }
    let recall = total / QUERIES as f64;
    assert!(
        recall >= 0.90,
        "post-delete recall@{K} = {recall:.3} below the 0.90 gate"
    );
}
