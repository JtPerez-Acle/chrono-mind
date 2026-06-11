//! Reference HNSW implementation guarded by a single `RwLock`.
//!
//! This is the correctness baseline: a faithful, readable implementation of
//! the HNSW algorithm (Malkov & Yashunin, 2018) with coarse locking. It
//! exists so the lock-free index can be verified against it and benchmarked
//! A/B; it is also a perfectly serviceable index for single-threaded use.
//!
//! Writes take the lock exclusively for the whole graph update. That is the
//! point — this implementation is *honestly* locked.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::sync::{Arc, RwLock};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{TotalF32, VectorIndex};
use crate::config::IndexParams;
use crate::metric::DistanceMetric;

/// Hard cap on layer indexes; `floor(-ln(u) * mL)` exceeds this only with
/// astronomically small probability, but the RNG should not be able to
/// allocate unbounded layer vectors.
const MAX_LAYER: usize = 31;

struct Node {
    vector: Box<[f32]>,
    /// `neighbors[l]` holds the ids connected at layer `l`;
    /// `neighbors.len() - 1` is the node's top layer.
    neighbors: Vec<Vec<u32>>,
    deleted: bool,
}

impl Node {
    fn top_layer(&self) -> usize {
        self.neighbors.len() - 1
    }
}

struct Inner {
    nodes: Vec<Node>,
    /// Node with the highest top layer; the global entry point.
    entry: Option<u32>,
    live: usize,
    rng: StdRng,
}

/// A coarse-grained locked HNSW index. See the module docs.
pub struct RwLockHnsw {
    params: IndexParams,
    metric: Arc<dyn DistanceMetric>,
    /// Level normalization factor `mL = 1 / ln(M)`.
    ml: f64,
    inner: RwLock<Inner>,
}

impl RwLockHnsw {
    /// Create an index with an entropy-seeded layer RNG.
    pub fn new(params: IndexParams, metric: Arc<dyn DistanceMetric>) -> Self {
        Self::with_seed(params, metric, rand::random())
    }

    /// Create an index with a fixed RNG seed (deterministic layer
    /// assignment; used by tests and benchmarks).
    pub fn with_seed(params: IndexParams, metric: Arc<dyn DistanceMetric>, seed: u64) -> Self {
        let ml = 1.0 / (params.max_connections as f64).ln();
        Self {
            params,
            metric,
            ml,
            inner: RwLock::new(Inner {
                nodes: Vec::new(),
                entry: None,
                live: 0,
                rng: StdRng::seed_from_u64(seed),
            }),
        }
    }

    /// Maximum allowed connections at `layer` (`2M` at layer 0, `M` above).
    fn max_connections(&self, layer: usize) -> usize {
        if layer == 0 {
            self.params.max_connections * 2
        } else {
            self.params.max_connections
        }
    }

    /// Draw a top layer: `floor(-ln(u) * mL)`, capped at [`MAX_LAYER`].
    fn random_layer(&self, rng: &mut StdRng) -> usize {
        let u: f64 = rng.gen_range(f64::MIN_POSITIVE..1.0);
        ((-u.ln() * self.ml) as usize).min(MAX_LAYER)
    }

    /// Greedy best-first search within one layer (Algorithm 2 of the paper).
    ///
    /// Explores from `entry_points`, keeping a min-heap frontier of
    /// candidates and a bounded max-heap of the `ef` best results. Returns
    /// `(distance, id)` pairs sorted ascending. Read-only on the graph.
    fn search_layer(
        &self,
        nodes: &[Node],
        query: &[f32],
        entry_points: &[u32],
        ef: usize,
        layer: usize,
    ) -> Vec<(TotalF32, u32)> {
        let mut visited: HashSet<u32> = HashSet::new();
        // Min-heap of candidates to expand (closest first).
        let mut frontier: BinaryHeap<Reverse<(TotalF32, u32)>> = BinaryHeap::new();
        // Max-heap of the best `ef` results (worst on top, popped on overflow).
        let mut best: BinaryHeap<(TotalF32, u32)> = BinaryHeap::new();

        for &ep in entry_points {
            if visited.insert(ep) {
                let d = TotalF32(self.metric.distance(&nodes[ep as usize].vector, query));
                frontier.push(Reverse((d, ep)));
                best.push((d, ep));
            }
        }
        while best.len() > ef {
            best.pop();
        }

        while let Some(Reverse((dist, node))) = frontier.pop() {
            if let Some(&(worst, _)) = best.peek() {
                if best.len() >= ef && dist > worst {
                    break; // every remaining candidate is farther than all results
                }
            }
            for &neighbor in &nodes[node as usize].neighbors[layer] {
                if !visited.insert(neighbor) {
                    continue;
                }
                let d = TotalF32(
                    self.metric
                        .distance(&nodes[neighbor as usize].vector, query),
                );
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

    /// Greedy descent through `layer`, returning the closest node found.
    fn descend_layer(&self, nodes: &[Node], query: &[f32], entry: u32, layer: usize) -> u32 {
        self.search_layer(nodes, query, &[entry], 1, layer)
            .first()
            .map(|&(_, id)| id)
            .unwrap_or(entry)
    }

    /// Diversity-aware neighbor selection (Algorithm 4 of the paper, with
    /// `keepPrunedConnections`).
    ///
    /// A candidate is selected only if it is closer to the query than to
    /// every already-selected neighbor; this favors links that span
    /// different directions over redundant links into one tight cluster,
    /// which is what keeps the graph navigable in high dimensions. Slots
    /// left over are backfilled with the nearest rejected candidates.
    ///
    /// `candidates` must be sorted by ascending distance to the query.
    fn select_neighbors(
        &self,
        nodes: &[Node],
        candidates: &[(TotalF32, u32)],
        m: usize,
    ) -> Vec<u32> {
        let mut selected: Vec<(TotalF32, u32)> = Vec::with_capacity(m);
        let mut rejected: Vec<u32> = Vec::new();

        for &(dist_to_query, candidate) in candidates {
            if selected.len() >= m {
                break;
            }
            let candidate_vector = &nodes[candidate as usize].vector;
            let diverse = selected.iter().all(|&(_, chosen)| {
                let dist_to_chosen = self
                    .metric
                    .distance(candidate_vector, &nodes[chosen as usize].vector);
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

    /// Re-prune `node`'s connections at `layer` down to the allowed
    /// maximum using diversity-aware selection.
    fn prune(&self, inner: &mut Inner, node: u32, layer: usize) {
        let allowed = self.max_connections(layer);
        if inner.nodes[node as usize].neighbors[layer].len() <= allowed {
            return;
        }
        let node_vector = &inner.nodes[node as usize].vector;
        let mut by_distance: Vec<(TotalF32, u32)> = inner.nodes[node as usize].neighbors[layer]
            .iter()
            .map(|&n| {
                (
                    TotalF32(
                        self.metric
                            .distance(&inner.nodes[n as usize].vector, node_vector),
                    ),
                    n,
                )
            })
            .collect();
        by_distance.sort();
        let kept = self.select_neighbors(&inner.nodes, &by_distance, allowed);
        inner.nodes[node as usize].neighbors[layer] = kept;
    }
}

impl VectorIndex for RwLockHnsw {
    fn insert(&self, vector: &[f32]) -> Option<u32> {
        let mut inner = self.inner.write().expect("index lock poisoned");
        let inner = &mut *inner;
        if inner.nodes.len() >= u32::MAX as usize {
            return None; // u32 handle space exhausted
        }

        let id = inner.nodes.len() as u32;
        let top_layer = self.random_layer(&mut inner.rng);

        let Some(entry) = inner.entry else {
            // First node: it becomes the entry point at its own top layer.
            inner.nodes.push(Node {
                vector: vector.into(),
                neighbors: vec![Vec::new(); top_layer + 1],
                deleted: false,
            });
            inner.entry = Some(id);
            inner.live += 1;
            return Some(id);
        };

        let entry_layer = inner.nodes[entry as usize].top_layer();

        // Phase 1 (read-only): find this node's neighbors at every layer it
        // participates in, descending greedily through the layers above.
        let mut ep = entry;
        for layer in (top_layer + 1..=entry_layer).rev() {
            ep = self.descend_layer(&inner.nodes, vector, ep, layer);
        }

        let mut entry_points = vec![ep];
        let mut selected_per_layer: Vec<(usize, Vec<u32>)> = Vec::new();
        for layer in (0..=top_layer.min(entry_layer)).rev() {
            let candidates = self.search_layer(
                &inner.nodes,
                vector,
                &entry_points,
                self.params.ef_construction,
                layer,
            );
            let selected =
                self.select_neighbors(&inner.nodes, &candidates, self.params.max_connections);
            selected_per_layer.push((layer, selected));
            // The full candidate set seeds the next (lower) layer's search.
            entry_points = candidates.into_iter().map(|(_, n)| n).collect();
        }

        // Phase 2 (mutating): publish the node and link both directions.
        let mut neighbors = vec![Vec::new(); top_layer + 1];
        for (layer, selected) in &selected_per_layer {
            neighbors[*layer] = selected.clone();
        }
        inner.nodes.push(Node {
            vector: vector.into(),
            neighbors,
            deleted: false,
        });

        for (layer, selected) in selected_per_layer {
            for n in selected {
                inner.nodes[n as usize].neighbors[layer].push(id);
                self.prune(inner, n, layer);
            }
        }

        if top_layer > entry_layer {
            inner.entry = Some(id);
        }
        inner.live += 1;
        Some(id)
    }

    fn remove(&self, id: u32) -> bool {
        let mut inner = self.inner.write().expect("index lock poisoned");
        match inner.nodes.get_mut(id as usize) {
            Some(node) if !node.deleted => {
                node.deleted = true;
                inner.live -= 1;
                true
            }
            _ => false,
        }
    }

    fn search(&self, query: &[f32], ef: usize) -> Vec<(u32, f32)> {
        let inner = self.inner.read().expect("index lock poisoned");
        let Some(entry) = inner.entry else {
            return Vec::new();
        };
        if ef == 0 {
            return Vec::new();
        }

        let mut ep = entry;
        for layer in (1..=inner.nodes[entry as usize].top_layer()).rev() {
            ep = self.descend_layer(&inner.nodes, query, ep, layer);
        }

        self.search_layer(&inner.nodes, query, &[ep], ef, 0)
            .into_iter()
            .filter(|&(_, id)| !inner.nodes[id as usize].deleted)
            .map(|(d, id)| (id, d.0))
            .collect()
    }

    fn len(&self) -> usize {
        self.inner.read().expect("index lock poisoned").live
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metric::CosineDistance;

    fn index() -> RwLockHnsw {
        RwLockHnsw::with_seed(IndexParams::default(), Arc::new(CosineDistance::new()), 42)
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
        let id = idx.insert(&[1.0, 0.0]).unwrap();
        let results = idx.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        assert!(results[0].1 < 1e-6);
    }

    #[test]
    fn nearest_is_ranked_first() {
        let idx = index();
        let near = idx.insert(&[1.0, 0.05]).unwrap();
        let _far = idx.insert(&[0.0, 1.0]).unwrap();
        let _mid = idx.insert(&[0.5, 0.5]).unwrap();

        let results = idx.search(&[1.0, 0.0], 3);
        assert_eq!(results[0].0, near);
        // Distances ascend.
        assert!(results.windows(2).all(|w| w[0].1 <= w[1].1));
    }

    #[test]
    fn tombstoned_vectors_disappear_from_results() {
        let idx = index();
        let a = idx.insert(&[1.0, 0.0]).unwrap();
        let b = idx.insert(&[0.9, 0.1]).unwrap();

        assert!(idx.remove(a));
        assert!(!idx.remove(a), "double remove reports false");
        assert_eq!(idx.len(), 1);

        let results = idx.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, b);
    }

    #[test]
    fn connections_respect_layer_caps() {
        let params = IndexParams {
            max_connections: 4,
            ef_construction: 16,
            ef_search: 16,
        };
        let idx = RwLockHnsw::with_seed(params, Arc::new(CosineDistance::new()), 7);
        let n = if cfg!(miri) { 40 } else { 200 };
        for i in 0..n {
            let angle = i as f32 * 0.05;
            idx.insert(&[angle.cos(), angle.sin()]).unwrap();
        }

        let inner = idx.inner.read().unwrap();
        for (i, node) in inner.nodes.iter().enumerate() {
            for (layer, neighbors) in node.neighbors.iter().enumerate() {
                let cap = idx.max_connections(layer);
                assert!(
                    neighbors.len() <= cap,
                    "node {i} layer {layer}: {} > cap {cap}",
                    neighbors.len()
                );
                for &n in neighbors {
                    assert!((n as usize) < inner.nodes.len(), "dangling neighbor id");
                    assert_ne!(n as usize, i, "self-loop");
                }
            }
        }
    }

    #[test]
    fn entry_point_tracks_highest_layer() {
        let idx = index();
        let n = if cfg!(miri) { 30 } else { 100 };
        for i in 0..n {
            let angle = i as f32 * 0.1;
            idx.insert(&[angle.cos(), angle.sin()]).unwrap();
        }
        let inner = idx.inner.read().unwrap();
        let entry = inner.entry.unwrap() as usize;
        let max_top = inner.nodes.iter().map(Node::top_layer).max().unwrap();
        assert_eq!(inner.nodes[entry].top_layer(), max_top);
    }

    #[test]
    fn seeded_indexes_are_deterministic() {
        let build = || {
            let idx = index();
            let n = if cfg!(miri) { 15 } else { 50 };
            for i in 0..n {
                let angle = i as f32 * 0.13;
                idx.insert(&[angle.cos(), angle.sin()]).unwrap();
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
}
