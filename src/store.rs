//! The [`ChronoMind`] store: temporal scoring, decay, contexts, and
//! relationships over a vector index.
//!
//! Searches go through an HNSW index (see [`crate::index`]) and are then
//! reranked by the temporal scoring formula documented on
//! [`ChronoMind::search`]. Methods that mutate take `&mut self` for now —
//! the lock-free index milestone relaxes writes to `&self`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::SystemTime;

use tracing::{debug, instrument};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::index::{RwLockHnsw, VectorIndex};
use crate::metric::{CosineDistance, DistanceMetric};
use crate::types::{ContextSummary, Memory, MemoryStats};

const SECONDS_PER_HOUR: f32 = 3600.0;

/// Oversampling factor: the index is asked for `max(ef_search, k * OVERSAMPLE)`
/// candidates before temporal reranking, so that recency can promote
/// memories that are geometrically close but not in the top `k`.
const OVERSAMPLE: usize = 3;

/// A temporal vector store.
///
/// See the [crate-level documentation](crate) for an end-to-end example.
pub struct ChronoMind {
    config: Config,
    metric: Arc<dyn DistanceMetric>,
    memories: HashMap<String, Memory>,
    index: RwLockHnsw,
    /// External id -> live index handle.
    handles: HashMap<String, u32>,
    /// Live index handle -> external id.
    names: HashMap<u32, String>,
}

impl std::fmt::Debug for ChronoMind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChronoMind")
            .field("config", &self.config)
            .field("metric", &self.metric.name())
            .field("memories", &self.memories.len())
            .finish()
    }
}

impl ChronoMind {
    /// Create a store with the given configuration and cosine distance.
    pub fn new(config: Config) -> Result<Self> {
        Self::with_metric(config, Arc::new(CosineDistance::new()))
    }

    /// Create a store with a custom distance metric.
    pub fn with_metric(config: Config, metric: Arc<dyn DistanceMetric>) -> Result<Self> {
        config.validate()?;
        let index = RwLockHnsw::new(config.index.clone(), Arc::clone(&metric));
        Ok(Self {
            config,
            metric,
            memories: HashMap::new(),
            index,
            handles: HashMap::new(),
            names: HashMap::new(),
        })
    }

    /// The store's configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Number of stored memories.
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    /// Insert a memory, replacing any existing memory with the same id.
    ///
    /// When replacing, relationship links from the previous memory are
    /// merged into the new one (deduplicated, capped at
    /// [`max_relationships`](Config::max_relationships)).
    #[instrument(skip(self, memory), fields(id = %memory.vector.id))]
    pub fn insert(&mut self, mut memory: Memory) -> Result<()> {
        memory.validate(&self.config)?;

        if let Some(existing) = self.memories.get(&memory.vector.id) {
            let mut links: Vec<String> = existing.attributes.relationships.clone();
            let known: HashSet<&String> = links.iter().collect();
            let new_links: Vec<String> = memory
                .attributes
                .relationships
                .iter()
                .filter(|l| !known.contains(l))
                .cloned()
                .collect();
            links.extend(new_links);
            links.truncate(self.config.max_relationships);
            memory.attributes.relationships = links;
            self.unindex(&memory.vector.id);
        } else if self.memories.len() >= self.config.max_memories {
            return Err(Error::CapacityExceeded(self.config.max_memories));
        } else {
            memory
                .attributes
                .relationships
                .truncate(self.config.max_relationships);
        }

        let handle = self.index.insert(&memory.vector.data);
        self.handles.insert(memory.vector.id.clone(), handle);
        self.names.insert(handle, memory.vector.id.clone());
        self.memories.insert(memory.vector.id.clone(), memory);
        Ok(())
    }

    /// Tombstone `id`'s entry in the index, if it has one.
    fn unindex(&mut self, id: &str) {
        if let Some(handle) = self.handles.remove(id) {
            self.names.remove(&handle);
            self.index.remove(handle);
        }
    }

    /// Get a memory by id.
    pub fn get(&self, id: &str) -> Option<Memory> {
        self.memories.get(id).cloned()
    }

    /// Get a memory by id, recording the access (bumps
    /// [`access_count`](crate::MemoryAttributes::access_count) and
    /// refreshes [`last_access`](crate::MemoryAttributes::last_access)).
    pub fn access(&mut self, id: &str) -> Option<Memory> {
        let memory = self.memories.get_mut(id)?;
        memory.attributes.access_count += 1;
        memory.attributes.last_access = SystemTime::now();
        Some(memory.clone())
    }

    /// Remove a memory by id, returning it if present.
    pub fn remove(&mut self, id: &str) -> Option<Memory> {
        let removed = self.memories.remove(id);
        if removed.is_some() {
            self.unindex(id);
        }
        removed
    }

    /// Iterate over all stored memories in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = &Memory> {
        self.memories.values()
    }

    /// Search for the `k` memories most relevant to `query`.
    ///
    /// Relevance combines geometric and temporal closeness. With
    /// `w = temporal_weight`, age `t` in hours, and per-memory decay rate
    /// `r` (falling back to `base_decay_rate` when zero):
    ///
    /// ```text
    /// score = (1 - w) * distance / 2 + w * (1 - exp(-r * t))
    /// ```
    ///
    /// Lower scores are better. Results are `(memory, score)` pairs sorted
    /// ascending. This formula is the single definition of temporal
    /// relevance used everywhere in the crate.
    ///
    /// The index supplies `max(ef_search, 3 * k)` geometric candidates and
    /// the formula reranks those; a memory outside that candidate pool
    /// cannot be returned, however fresh. Raise
    /// [`ef_search`](crate::IndexParams::ef_search) to widen the pool.
    #[instrument(skip(self, query))]
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(Memory, f32)>> {
        self.validate_query(query)?;
        let ef = self.config.index.ef_search.max(k * OVERSAMPLE);
        let now = SystemTime::now();

        let mut scored: Vec<(Memory, f32)> = self
            .index
            .search(query, ef)
            .into_iter()
            .filter_map(|(handle, distance)| {
                let id = self.names.get(&handle)?;
                let memory = self.memories.get(id)?;
                Some((memory.clone(), self.combined_score(distance, memory, now)))
            })
            .collect();

        scored.sort_by(|(_, a), (_, b)| a.total_cmp(b));
        scored.truncate(k);
        Ok(scored)
    }

    /// Like [`search`](Self::search), restricted to one context label.
    ///
    /// Context filtering scans the context's members exactly rather than
    /// going through the index, so sparse contexts never come back short.
    #[instrument(skip(self, query))]
    pub fn search_in_context(
        &self,
        context: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(Memory, f32)>> {
        self.validate_query(query)?;
        let now = SystemTime::now();

        let mut scored: Vec<(Memory, f32)> = self
            .memories
            .values()
            .filter(|m| m.attributes.context == context)
            .map(|m| {
                let distance = self.metric.distance(&m.vector.data, query);
                (m.clone(), self.combined_score(distance, m, now))
            })
            .collect();

        scored.sort_by(|(_, a), (_, b)| a.total_cmp(b));
        scored.truncate(k);
        Ok(scored)
    }

    fn validate_query(&self, query: &[f32]) -> Result<()> {
        if query.len() != self.config.dimensions {
            return Err(Error::InvalidDimensions {
                got: query.len(),
                expected: self.config.dimensions,
            });
        }
        if query.iter().any(|x| !x.is_finite()) {
            return Err(Error::InvalidVector(
                "query contains NaN or infinite components".into(),
            ));
        }
        Ok(())
    }

    /// The single temporal scoring formula. See [`search`](Self::search).
    fn combined_score(&self, distance: f32, memory: &Memory, now: SystemTime) -> f32 {
        let w = self.config.temporal_weight;
        let age_hours = memory.age(now).as_secs_f32() / SECONDS_PER_HOUR;
        let rate = self.effective_decay_rate(memory);
        let temporal_relevance = (-rate * age_hours).exp(); // 1 = fresh, 0 = ancient
        (1.0 - w) * (distance / 2.0) + w * (1.0 - temporal_relevance)
    }

    fn effective_decay_rate(&self, memory: &Memory) -> f32 {
        if memory.attributes.decay_rate > 0.0 {
            memory.attributes.decay_rate
        } else {
            self.config.base_decay_rate
        }
    }

    /// Decay every memory's importance based on time since last access.
    ///
    /// Importance is multiplied by `exp(-r * h)` where `r` is the memory's
    /// effective decay rate and `h` is hours since
    /// [`last_access`](crate::MemoryAttributes::last_access), then clamped
    /// to `[0.0, 1.0]`. Call periodically; the operation is idempotent for
    /// a fixed instant.
    #[instrument(skip(self))]
    pub fn apply_decay(&mut self) {
        let now = SystemTime::now();
        for memory in self.memories.values_mut() {
            let hours = now
                .duration_since(memory.attributes.last_access)
                .unwrap_or_default()
                .as_secs_f32()
                / SECONDS_PER_HOUR;
            let rate = if memory.attributes.decay_rate > 0.0 {
                memory.attributes.decay_rate
            } else {
                self.config.base_decay_rate
            };
            let factor = (-rate * hours).exp();
            memory.attributes.importance = (memory.attributes.importance * factor).clamp(0.0, 1.0);
        }
    }

    /// Merge near-duplicate memories.
    ///
    /// For every pair with cosine similarity above
    /// [`similarity_threshold`](Config::similarity_threshold), the
    /// lower-importance memory is absorbed into the higher-importance one:
    /// relationships merge, importance keeps the maximum, and the absorbed
    /// memory is removed. Returns the number of memories absorbed.
    ///
    /// Complexity is `O(n²)` in the number of memories; intended as a
    /// maintenance operation, not a hot-path call.
    #[instrument(skip(self))]
    pub fn consolidate(&mut self) -> usize {
        let ids: Vec<String> = self.memories.keys().cloned().collect();
        let mut absorbed: HashSet<String> = HashSet::new();

        for i in 0..ids.len() {
            if absorbed.contains(&ids[i]) {
                continue;
            }
            for j in (i + 1)..ids.len() {
                if absorbed.contains(&ids[j]) {
                    continue;
                }
                let (a, b) = (&self.memories[&ids[i]], &self.memories[&ids[j]]);
                let similarity = self.metric.similarity(&a.vector.data, &b.vector.data);
                if similarity <= self.config.similarity_threshold {
                    continue;
                }

                // Keep the more important memory; absorb the other.
                let (keep_id, drop_id) = if a.attributes.importance >= b.attributes.importance {
                    (ids[i].clone(), ids[j].clone())
                } else {
                    (ids[j].clone(), ids[i].clone())
                };
                let dropped = self
                    .memories
                    .remove(&drop_id)
                    .expect("id listed and not yet absorbed");
                self.unindex(&drop_id);
                let keeper = self
                    .memories
                    .get_mut(&keep_id)
                    .expect("id listed and not yet absorbed");

                let known: HashSet<String> =
                    keeper.attributes.relationships.iter().cloned().collect();
                for link in dropped.attributes.relationships {
                    if link != keep_id && !known.contains(&link) {
                        keeper.attributes.relationships.push(link);
                    }
                }
                keeper
                    .attributes
                    .relationships
                    .truncate(self.config.max_relationships);
                keeper.attributes.importance = keeper
                    .attributes
                    .importance
                    .max(dropped.attributes.importance);

                debug!(kept = %keep_id, dropped = %drop_id, similarity, "consolidated");
                absorbed.insert(drop_id);
                if absorbed.contains(&ids[i]) {
                    break; // memory i was the one absorbed; stop pairing it
                }
            }
        }

        absorbed.len()
    }

    /// Memories reachable from `id` by following relationship links, up to
    /// `max_depth` hops, in breadth-first order. The starting memory is not
    /// included.
    pub fn related(&self, id: &str, max_depth: usize) -> Vec<Memory> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        visited.insert(id.to_string());
        queue.push_back((id.to_string(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let Some(memory) = self.memories.get(&current) else {
                continue;
            };
            for link in &memory.attributes.relationships {
                if visited.insert(link.clone()) {
                    if let Some(linked) = self.memories.get(link) {
                        result.push(linked.clone());
                        queue.push_back((link.clone(), depth + 1));
                    }
                }
            }
        }

        result
    }

    /// Summarize the memories sharing a context label, or `None` if the
    /// context is empty.
    pub fn context_summary(&self, context: &str) -> Option<ContextSummary> {
        let members: Vec<&Memory> = self
            .memories
            .values()
            .filter(|m| m.attributes.context == context)
            .collect();
        if members.is_empty() {
            return None;
        }

        let count = members.len();
        let mut centroid = vec![0.0f32; self.config.dimensions];
        let mut importance_sum = 0.0f32;
        for m in &members {
            importance_sum += m.attributes.importance;
            for (acc, x) in centroid.iter_mut().zip(&m.vector.data) {
                *acc += x;
            }
        }
        for acc in &mut centroid {
            *acc /= count as f32;
        }

        Some(ContextSummary {
            context: context.to_string(),
            memory_count: count,
            average_importance: importance_sum / count as f32,
            centroid,
        })
    }

    /// Aggregate statistics for the store.
    pub fn stats(&self) -> MemoryStats {
        let total = self.memories.len();
        let mut total_components = 0usize;
        let mut importance_sum = 0.0f32;
        let mut contexts: HashMap<String, usize> = HashMap::new();
        let mut references: HashMap<String, usize> = HashMap::new();

        for m in self.memories.values() {
            total_components += m.vector.data.len();
            importance_sum += m.attributes.importance;
            *contexts.entry(m.attributes.context.clone()).or_insert(0) += 1;
            for link in &m.attributes.relationships {
                *references.entry(link.clone()).or_insert(0) += 1;
            }
        }

        let mut most_referenced: Vec<(String, usize)> = references.into_iter().collect();
        most_referenced.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        most_referenced.truncate(10);

        MemoryStats {
            total_memories: total,
            total_components,
            capacity_used: total as f64 / self.config.max_memories as f64,
            average_importance: if total > 0 {
                importance_sum / total as f32
            } else {
                0.0
            },
            context_distribution: contexts,
            most_referenced,
        }
    }
}
