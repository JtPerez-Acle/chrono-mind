//! A sharded-lock HNSW baseline: the fair competitor.
//!
//! A single `RwLock` is the baseline everyone beats; the version a
//! practitioner would actually deploy shards the index across many
//! independently locked HNSW graphs, routing inserts round-robin. Writes
//! then contend only within a shard, which is how locked designs claw
//! back write scalability.
//!
//! The cost moves to reads: a query must search **every** shard and merge,
//! so a 16-shard index does roughly 16 small graph searches per query —
//! more total distance computations than one search of a single large
//! graph, and a read still blocks whenever its shard takes a write. The
//! benchmarks quantify both sides of that trade against the lock-free
//! index.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{RwLockHnsw, TotalF32, VectorIndex};
use crate::config::IndexParams;
use crate::metric::DistanceMetric;

/// Number of shards. 16 comfortably exceeds the benchmarked thread counts,
/// so write contention per shard is low.
const SHARDS: usize = 16;
const SHARD_BITS: u32 = 4;

/// HNSW sharded over `SHARDS` (16) independent `RwLock`-guarded graphs.
/// See the module docs.
pub struct ShardedRwLockHnsw {
    shards: Vec<RwLockHnsw>,
    /// Round-robin insert router.
    ticket: AtomicUsize,
}

impl ShardedRwLockHnsw {
    /// Create a sharded index with an entropy-seeded layer RNG.
    pub fn new(params: IndexParams, metric: Arc<dyn DistanceMetric>) -> Self {
        Self::with_seed(params, metric, rand::random())
    }

    /// Create a sharded index with deterministic per-shard seeds.
    pub fn with_seed(params: IndexParams, metric: Arc<dyn DistanceMetric>, seed: u64) -> Self {
        Self {
            shards: (0..SHARDS)
                .map(|i| {
                    RwLockHnsw::with_seed(params.clone(), Arc::clone(&metric), seed ^ (i as u64))
                })
                .collect(),
            ticket: AtomicUsize::new(0),
        }
    }

    fn split(handle: u32) -> (usize, u32) {
        (
            (handle & (SHARDS as u32 - 1)) as usize,
            handle >> SHARD_BITS,
        )
    }

    fn join_handle(shard: usize, local: u32) -> Option<u32> {
        if local >= (1 << (32 - SHARD_BITS)) {
            return None; // local handle space exhausted
        }
        Some((local << SHARD_BITS) | shard as u32)
    }
}

impl VectorIndex for ShardedRwLockHnsw {
    fn insert(&self, vector: &[f32]) -> Option<u32> {
        let shard = self.ticket.fetch_add(1, Ordering::Relaxed) % SHARDS;
        let local = self.shards[shard].insert(vector)?;
        Self::join_handle(shard, local)
    }

    fn remove(&self, handle: u32) -> bool {
        let (shard, local) = Self::split(handle);
        self.shards[shard].remove(local)
    }

    fn search(&self, query: &[f32], ef: usize) -> Vec<(u32, f32)> {
        // Scatter the query to every shard, gather and merge by distance.
        let mut merged: Vec<(TotalF32, u32)> = Vec::with_capacity(ef * SHARDS);
        for (shard, index) in self.shards.iter().enumerate() {
            for (local, dist) in index.search(query, ef) {
                let global = Self::join_handle(shard, local)
                    .expect("existing local handles always re-encode");
                merged.push((TotalF32(dist), global));
            }
        }
        merged.sort();
        merged.truncate(ef);
        merged.into_iter().map(|(d, id)| (id, d.0)).collect()
    }

    fn len(&self) -> usize {
        self.shards.iter().map(|s| s.len()).sum()
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use crate::metric::CosineDistance;

    fn index() -> ShardedRwLockHnsw {
        ShardedRwLockHnsw::with_seed(IndexParams::default(), Arc::new(CosineDistance::new()), 42)
    }

    #[test]
    fn roundtrip_across_shards() {
        let idx = index();
        let n = if cfg!(miri) { 20 } else { 100 };
        let handles: Vec<u32> = (0..n)
            .map(|i| {
                let angle = i as f32 * 0.07;
                idx.insert(&[angle.cos(), angle.sin()]).unwrap()
            })
            .collect();
        assert_eq!(idx.len(), n);
        // Handles route back to distinct shards.
        let shards_used: std::collections::HashSet<u32> = handles.iter().map(|h| h & 15).collect();
        assert!(shards_used.len() > 1, "round-robin must spread shards");

        // Every direction is findable as its own best match.
        for i in 0..n {
            let angle = i as f32 * 0.07;
            let results = idx.search(&[angle.cos(), angle.sin()], 10);
            assert!(results[0].1 < 1e-4);
            assert!(results.windows(2).all(|w| w[0].1 <= w[1].1), "unsorted");
        }
    }

    #[test]
    fn remove_routes_to_the_right_shard() {
        let idx = index();
        let a = idx.insert(&[1.0, 0.0]).unwrap();
        let b = idx.insert(&[0.9, 0.1]).unwrap();
        assert!(idx.remove(a));
        assert!(!idx.remove(a));
        assert_eq!(idx.len(), 1);
        let results = idx.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, b);
    }
}
