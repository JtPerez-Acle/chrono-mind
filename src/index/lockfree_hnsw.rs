//! The lock-free HNSW index.
//!
//! Same algorithm as [`RwLockHnsw`](super::RwLockHnsw) — standard HNSW with
//! Algorithm 4 neighbor selection — built on lock-free foundations instead
//! of a lock:
//!
//! | concern            | mechanism                                      |
//! |--------------------|------------------------------------------------|
//! | node storage       | [`Arena`]: chunked, append-only, never moves   |
//! | adjacency          | [`NeighborList`]: COW slices + epoch GC        |
//! | entry point        | one packed `AtomicU64`, CAS-updated            |
//! | deletion           | per-node tombstone `AtomicBool`                |
//! | layer assignment   | SplitMix64 over an atomic counter              |
//!
//! **Progress guarantees.** Searches are wait-free: epoch-pinned pointer
//! loads only — no CAS, no retry, no lock, regardless of concurrent writer
//! activity. Inserts and removals are lock-free: every CAS retry is caused
//! by another operation having succeeded, so the system always makes
//! progress; no operation ever blocks.
//!
//! **What concurrency changes about results.** Two inserts running at the
//! same instant cannot link to each other (neither is published while both
//! search for neighbors), so concurrently built graphs can differ from
//! sequentially built ones. This affects construction interleaving, not
//! correctness: the stress and recall gates verify graph invariants and
//! recall over concurrently built indexes.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_epoch::{self as epoch, Guard};

use super::arena::Arena;
use super::neighbors::NeighborList;
use super::{TotalF32, VectorIndex};
use crate::config::IndexParams;
use crate::metric::DistanceMetric;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

/// Layer cap, as in the baseline.
const MAX_LAYER: usize = 31;

/// Sentinel for "no entry point yet".
const EMPTY_ENTRY: u64 = u64::MAX;

struct Node {
    vector: Box<[f32]>,
    top_layer: usize,
    deleted: AtomicBool,
    /// One COW list per layer `0..=top_layer`.
    layers: Box<[NeighborList]>,
}

/// Pack `(top_layer, id)` into the entry word.
fn pack_entry(id: u32, top_layer: usize) -> u64 {
    ((top_layer as u64) << 32) | id as u64
}

fn unpack_entry(word: u64) -> Option<(u32, usize)> {
    if word == EMPTY_ENTRY {
        None
    } else {
        Some((word as u32, (word >> 32) as usize))
    }
}

/// SplitMix64: tiny, high-quality mixer for layer assignment. One atomic
/// increment per insert; no shared RNG state to lock.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// The lock-free HNSW index. See the module docs.
pub struct LockFreeHnsw {
    params: IndexParams,
    metric: Arc<dyn DistanceMetric>,
    /// Level normalization factor `mL = 1 / ln(M)`.
    ml: f64,
    nodes: Arena<Node>,
    /// Packed `(top_layer << 32) | id`, or [`EMPTY_ENTRY`].
    entry: AtomicU64,
    live: AtomicUsize,
    /// Monotone counter feeding SplitMix64 for layer draws.
    layer_ticket: AtomicU64,
    seed: u64,
}

impl LockFreeHnsw {
    /// Create an index with an entropy-derived layer seed.
    pub fn new(params: IndexParams, metric: Arc<dyn DistanceMetric>) -> Self {
        Self::with_seed(params, metric, rand::random())
    }

    /// Create an index with a fixed layer seed (deterministic layer
    /// assignment under single-threaded insertion; used by tests and
    /// benchmarks).
    pub fn with_seed(params: IndexParams, metric: Arc<dyn DistanceMetric>, seed: u64) -> Self {
        let ml = 1.0 / (params.max_connections as f64).ln();
        Self {
            params,
            metric,
            ml,
            nodes: Arena::new(),
            entry: AtomicU64::new(EMPTY_ENTRY),
            live: AtomicUsize::new(0),
            layer_ticket: AtomicU64::new(0),
            seed,
        }
    }

    fn max_connections(&self, layer: usize) -> usize {
        if layer == 0 {
            self.params.max_connections * 2
        } else {
            self.params.max_connections
        }
    }

    /// Draw a top layer: `floor(-ln(u) * mL)`, capped at [`MAX_LAYER`].
    fn random_layer(&self) -> usize {
        let ticket = self.layer_ticket.fetch_add(1, Ordering::Relaxed);
        let bits = splitmix64(self.seed ^ ticket);
        // 53 high bits -> uniform in [0, 1); flip to (0, 1] to keep ln finite.
        let u = 1.0 - (bits >> 11) as f64 / (1u64 << 53) as f64;
        ((-u.ln() * self.ml) as usize).min(MAX_LAYER)
    }

    fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Greedy best-first search within one layer. Wait-free: epoch-pinned
    /// loads of COW neighbor slices, no writes anywhere.
    fn search_layer(
        &self,
        query: &[f32],
        entry_points: &[u32],
        ef: usize,
        layer: usize,
        guard: &Guard,
    ) -> Vec<(TotalF32, u32)> {
        let mut visited: HashSet<u32> = HashSet::new();
        let mut frontier: BinaryHeap<Reverse<(TotalF32, u32)>> = BinaryHeap::new();
        let mut best: BinaryHeap<(TotalF32, u32)> = BinaryHeap::new();

        for &ep in entry_points {
            let Some(node) = self.node(ep) else { continue };
            if layer > node.top_layer || !visited.insert(ep) {
                continue;
            }
            let d = TotalF32(self.metric.distance(&node.vector, query));
            frontier.push(Reverse((d, ep)));
            best.push((d, ep));
        }
        while best.len() > ef {
            best.pop();
        }

        while let Some(Reverse((dist, current))) = frontier.pop() {
            if let Some(&(worst, _)) = best.peek() {
                if best.len() >= ef && dist > worst {
                    break;
                }
            }
            let Some(node) = self.node(current) else {
                continue;
            };
            for &neighbor in node.layers[layer].load(guard) {
                if !visited.insert(neighbor) {
                    continue;
                }
                let Some(neighbor_node) = self.node(neighbor) else {
                    continue;
                };
                let d = TotalF32(self.metric.distance(&neighbor_node.vector, query));
                let admit = best.len() < ef || d < best.peek().expect("non-empty").0;
                if admit {
                    frontier.push(Reverse((d, neighbor)));
                    best.push((d, neighbor));
                    if best.len() > ef {
                        best.pop();
                    }
                }
            }
        }

        best.into_sorted_vec()
    }

    fn descend_layer(&self, query: &[f32], entry: u32, layer: usize, guard: &Guard) -> u32 {
        self.search_layer(query, &[entry], 1, layer, guard)
            .first()
            .map(|&(_, id)| id)
            .unwrap_or(entry)
    }

    /// Algorithm 4 diversity selection over candidates sorted by ascending
    /// distance to the query (with `keepPrunedConnections` backfill).
    fn select_diverse(&self, candidates: &[(TotalF32, u32)], m: usize) -> Vec<u32> {
        let mut selected: Vec<(TotalF32, u32)> = Vec::with_capacity(m);
        let mut rejected: Vec<u32> = Vec::new();

        for &(dist_to_query, candidate) in candidates {
            if selected.len() >= m {
                break;
            }
            let Some(candidate_node) = self.node(candidate) else {
                continue;
            };
            let diverse = selected.iter().all(|&(_, chosen)| {
                let Some(chosen_node) = self.node(chosen) else {
                    return true;
                };
                let dist_to_chosen = self
                    .metric
                    .distance(&candidate_node.vector, &chosen_node.vector);
                dist_to_query.0 < dist_to_chosen
            });
            if diverse {
                selected.push((dist_to_query, candidate));
            } else {
                rejected.push(candidate);
            }
        }

        let mut result: Vec<u32> = selected.into_iter().map(|(_, n)| n).collect();
        for candidate in rejected {
            if result.len() >= m {
                break;
            }
            result.push(candidate);
        }
        result
    }

    /// Publish a backlink `from -> to` at `layer`, pruning diversely if the
    /// list overflows. Lock-free CAS-retry via [`NeighborList::update`];
    /// the closure's intent ("ensure `to` is present") is idempotent, so
    /// retries over fresh values are sound.
    fn add_backlink(&self, from: u32, to: u32, layer: usize, guard: &Guard) {
        let Some(from_node) = self.node(from) else {
            return;
        };
        let cap = self.max_connections(layer);
        from_node.layers[layer].update(guard, |current| {
            if current.contains(&to) {
                return None; // already linked (a prior attempt's CAS won)
            }
            if current.len() < cap {
                let mut grown = Vec::with_capacity(current.len() + 1);
                grown.extend_from_slice(current);
                grown.push(to);
                return Some(grown);
            }
            // Overflow: re-select diversely among current + new, by
            // distance to `from`.
            let mut with_distances: Vec<(TotalF32, u32)> = current
                .iter()
                .chain(std::iter::once(&to))
                .filter_map(|&id| {
                    let node = self.node(id)?;
                    Some((
                        TotalF32(self.metric.distance(&node.vector, &from_node.vector)),
                        id,
                    ))
                })
                .collect();
            with_distances.sort();
            Some(self.select_diverse(&with_distances, cap))
        });
    }

    /// Current entry point as `(id, top_layer)`.
    fn entry_point(&self) -> Option<(u32, usize)> {
        unpack_entry(self.entry.load(Ordering::Acquire))
    }

    /// Walk the whole graph and verify structural invariants. Used by the
    /// stress-test gate; not part of the stable API.
    ///
    /// Checks, for every node and layer: neighbor handles point inside the
    /// arena, no neighbor list contains duplicates or self-loops, no list
    /// exceeds its connection cap, and the entry point (if set) is a valid
    /// node at its claimed layer.
    #[doc(hidden)]
    pub fn check_invariants(&self) -> std::result::Result<(), String> {
        let guard = epoch::pin();
        let total = self.nodes.len() as u32;

        if let Some((entry_id, entry_top)) = self.entry_point() {
            let node = self
                .node(entry_id)
                .ok_or_else(|| format!("entry {entry_id} not in arena"))?;
            if node.top_layer < entry_top {
                return Err(format!(
                    "entry {entry_id} claims layer {entry_top} but tops out at {}",
                    node.top_layer
                ));
            }
        }

        for id in 0..total {
            let Some(node) = self.node(id) else { continue };
            for (layer, list) in node.layers.iter().enumerate() {
                let ids = list.load(&guard);
                if ids.len() > self.max_connections(layer) {
                    return Err(format!(
                        "node {id} layer {layer}: {} links exceeds cap {}",
                        ids.len(),
                        self.max_connections(layer)
                    ));
                }
                let mut seen = HashSet::with_capacity(ids.len());
                for &n in ids {
                    if n == id {
                        return Err(format!("node {id} layer {layer}: self-loop"));
                    }
                    if n >= total {
                        return Err(format!(
                            "node {id} layer {layer}: dangling handle {n} (arena has {total})"
                        ));
                    }
                    if !seen.insert(n) {
                        return Err(format!("node {id} layer {layer}: duplicate link {n}"));
                    }
                    let neighbor = self
                        .node(n)
                        .ok_or_else(|| format!("node {id} layer {layer}: unreadable link {n}"))?;
                    if neighbor.top_layer < layer {
                        return Err(format!(
                            "node {id} layer {layer}: link {n} does not participate in layer"
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Raise the entry point to `(id, top_layer)` if it is higher than the
    /// current one. CAS loop; lock-free.
    fn raise_entry(&self, id: u32, top_layer: usize) {
        let mut current = self.entry.load(Ordering::Acquire);
        loop {
            let should_replace = match unpack_entry(current) {
                None => true,
                Some((_, current_top)) => top_layer > current_top,
            };
            if !should_replace {
                return;
            }
            match self.entry.compare_exchange_weak(
                current,
                pack_entry(id, top_layer),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }
}

impl VectorIndex for LockFreeHnsw {
    fn insert(&self, vector: &[f32]) -> u32 {
        let top_layer = self.random_layer();
        let id = self.nodes.push(Node {
            vector: vector.into(),
            top_layer,
            deleted: AtomicBool::new(false),
            layers: (0..=top_layer).map(|_| NeighborList::new()).collect(),
        });
        // The node is in the arena but unreachable: no inbound links, not
        // the entry point. Everything below publishes it.

        let guard = epoch::pin();

        if self.entry.load(Ordering::Acquire) == EMPTY_ENTRY {
            // Try to become the first entry; on failure fall through to a
            // normal linked insert against whoever won.
            if self
                .entry
                .compare_exchange(
                    EMPTY_ENTRY,
                    pack_entry(id, top_layer),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.live.fetch_add(1, Ordering::AcqRel);
                return id;
            }
        }

        let (entry_id, entry_top) = self
            .entry_point()
            .expect("entry is non-empty past the bootstrap branch");

        // Phase 1 (read-only): collect neighbor selections per layer.
        let mut ep = entry_id;
        for layer in (top_layer + 1..=entry_top).rev() {
            ep = self.descend_layer(vector, ep, layer, &guard);
        }

        let mut entry_points = vec![ep];
        let mut selected_per_layer: Vec<(usize, Vec<u32>)> = Vec::new();
        for layer in (0..=top_layer.min(entry_top)).rev() {
            let candidates = self.search_layer(
                vector,
                &entry_points,
                self.params.ef_construction,
                layer,
                &guard,
            );
            let selected = self.select_diverse(&candidates, self.params.max_connections);
            selected_per_layer.push((layer, selected));
            entry_points = candidates.into_iter().map(|(_, n)| n).collect();
        }

        // Phase 2: set our own lists (exclusive: the id is still private),
        // then publish inbound links and possibly the entry point.
        let node = self.node(id).expect("just pushed");
        for (layer, selected) in &selected_per_layer {
            node.layers[*layer].store(selected.clone(), &guard);
        }
        for (layer, selected) in selected_per_layer {
            for neighbor in selected {
                self.add_backlink(neighbor, id, layer, &guard);
            }
        }
        self.raise_entry(id, top_layer);

        self.live.fetch_add(1, Ordering::AcqRel);
        id
    }

    fn remove(&self, id: u32) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        // CAS rather than store: exactly one caller wins a concurrent
        // double-remove, keeping the live count accurate.
        if node
            .deleted
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.live.fetch_sub(1, Ordering::AcqRel);
            true
        } else {
            false
        }
    }

    fn search(&self, query: &[f32], ef: usize) -> Vec<(u32, f32)> {
        if ef == 0 {
            return Vec::new();
        }
        let guard = epoch::pin();
        let Some((entry_id, entry_top)) = self.entry_point() else {
            return Vec::new();
        };

        let mut ep = entry_id;
        for layer in (1..=entry_top).rev() {
            ep = self.descend_layer(query, ep, layer, &guard);
        }

        self.search_layer(query, &[ep], ef, 0, &guard)
            .into_iter()
            .filter(|&(_, id)| {
                self.node(id)
                    .map(|n| !n.deleted.load(Ordering::Acquire))
                    .unwrap_or(false)
            })
            .map(|(d, id)| (id, d.0))
            .collect()
    }

    fn len(&self) -> usize {
        self.live.load(Ordering::Acquire)
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use crate::metric::CosineDistance;

    fn index() -> LockFreeHnsw {
        LockFreeHnsw::with_seed(IndexParams::default(), Arc::new(CosineDistance::new()), 42)
    }

    #[test]
    fn empty_index_returns_nothing() {
        let idx = index();
        assert!(idx.search(&[1.0, 0.0], 10).is_empty());
        assert!(idx.is_empty());
    }

    #[test]
    fn single_vector_is_found() {
        let idx = index();
        let id = idx.insert(&[1.0, 0.0]);
        let results = idx.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        assert!(results[0].1 < 1e-6);
    }

    #[test]
    fn nearest_is_ranked_first() {
        let idx = index();
        let near = idx.insert(&[1.0, 0.05]);
        let _far = idx.insert(&[0.0, 1.0]);
        let _mid = idx.insert(&[0.5, 0.5]);

        let results = idx.search(&[1.0, 0.0], 3);
        assert_eq!(results[0].0, near);
        assert!(results.windows(2).all(|w| w[0].1 <= w[1].1));
    }

    #[test]
    fn tombstoned_vectors_disappear_from_results() {
        let idx = index();
        let a = idx.insert(&[1.0, 0.0]);
        let b = idx.insert(&[0.9, 0.1]);

        assert!(idx.remove(a));
        assert!(!idx.remove(a));
        assert_eq!(idx.len(), 1);

        let results = idx.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, b);
    }

    #[test]
    fn seeded_single_threaded_inserts_are_deterministic() {
        let build = || {
            let idx = index();
            let n = if cfg!(miri) { 15 } else { 50 };
            for i in 0..n {
                let angle = i as f32 * 0.13;
                idx.insert(&[angle.cos(), angle.sin()]);
            }
            idx.search(&[1.0, 0.0], 10)
        };
        let (a, b) = (build(), build());
        let ids = |r: &[(u32, f32)]| r.iter().map(|&(id, _)| id).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b), "construction must be deterministic");
        // Distances are bit-identical natively; Miri deliberately jitters
        // the last bit of float ops, so only the ranking is asserted there.
        #[cfg(not(miri))]
        assert_eq!(a, b);
    }

    #[test]
    fn concurrent_inserts_build_a_searchable_graph() {
        let idx = Arc::new(index());
        // Miri interprets every instruction; keep its workload tiny.
        let threads = if cfg!(miri) { 3 } else { 8 };
        let per_thread = if cfg!(miri) { 5 } else { 50 };

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let idx = Arc::clone(&idx);
                std::thread::spawn(move || {
                    for i in 0..per_thread {
                        let angle = (t * per_thread + i) as f32 * 0.01;
                        idx.insert(&[angle.cos(), angle.sin()]);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(idx.len(), threads * per_thread);
        // Every stored direction must be findable as its own best match.
        for probe in 0..(threads * per_thread) {
            let angle = probe as f32 * 0.01;
            let results = idx.search(&[angle.cos(), angle.sin()], 10);
            assert!(!results.is_empty());
            assert!(
                results[0].1 < 1e-4,
                "probe {probe}: best distance {}",
                results[0].1
            );
        }
    }
}
