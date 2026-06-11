//! The [`ChronoMind`] store: temporal scoring, decay, contexts, and
//! relationships over the lock-free vector index.
//!
//! The store is fully concurrent: every operation except
//! [`consolidate`](ChronoMind::consolidate) takes `&self` and can run from
//! any number of threads simultaneously. Nothing blocks on a mutex or
//! RwLock anywhere in the crate:
//!
//! - vector search goes through the lock-free HNSW index
//!   ([`crate::index::LockFreeHnsw`]) — wait-free reads, lock-free writes;
//! - id maps are `papaya` hash maps (lock-free reads, fine-grained
//!   lock-free updates);
//! - mutable memory state (importance, access tracking) lives in atomics
//!   inside immutable records, so decay sweeps and access bumps are
//!   CAS loops, not lock acquisitions.
//!
//! `consolidate` takes `&mut self` deliberately: it is an `O(n²)`
//! maintenance pass whose pairwise logic is not meaningfully concurrent,
//! and exclusive access keeps it trivially correct. That is an API choice,
//! not a hidden lock.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing::{debug, instrument};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::index::{LockFreeHnsw, VectorIndex};
use crate::metric::{CosineDistance, DistanceMetric};
use crate::types::{ContextSummary, Memory, MemoryAttributes, MemoryStats, Vector};

const SECONDS_PER_HOUR: f32 = 3600.0;

/// Oversampling factor: the index is asked for `max(ef_search, k * OVERSAMPLE)`
/// candidates before temporal reranking, so that recency can promote
/// memories that are geometrically close but not in the top `k`.
const OVERSAMPLE: usize = 3;

/// An immutable memory record with atomic mutable state.
///
/// Identity, vector data, and temporal constants never change after
/// creation (re-inserting an id replaces the whole record). Importance and
/// access tracking are atomics so they can be updated lock-free from any
/// thread.
struct StoredMemory {
    handle: u32,
    id: String,
    data: Vec<f32>,
    timestamp: SystemTime,
    context: String,
    decay_rate: f32,
    relationships: Box<[String]>,
    importance_bits: AtomicU32,
    access_count: AtomicU32,
    last_access_nanos: AtomicU64,
}

fn nanos_since_epoch(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

impl StoredMemory {
    fn from_memory(memory: &Memory, handle: u32) -> Arc<Self> {
        let a = &memory.attributes;
        Arc::new(Self {
            handle,
            id: memory.vector.id.clone(),
            data: memory.vector.data.clone(),
            timestamp: a.timestamp,
            context: a.context.clone(),
            decay_rate: a.decay_rate,
            relationships: a.relationships.clone().into_boxed_slice(),
            importance_bits: AtomicU32::new(a.importance.to_bits()),
            access_count: AtomicU32::new(a.access_count),
            last_access_nanos: AtomicU64::new(nanos_since_epoch(a.last_access)),
        })
    }

    /// Rebuild with different relationships/importance, preserving identity
    /// and access state (used by consolidation).
    fn rebuilt(&self, relationships: Vec<String>, importance: f32) -> Arc<Self> {
        Arc::new(Self {
            handle: self.handle,
            id: self.id.clone(),
            data: self.data.clone(),
            timestamp: self.timestamp,
            context: self.context.clone(),
            decay_rate: self.decay_rate,
            relationships: relationships.into_boxed_slice(),
            importance_bits: AtomicU32::new(importance.to_bits()),
            access_count: AtomicU32::new(self.access_count.load(Ordering::Acquire)),
            last_access_nanos: AtomicU64::new(self.last_access_nanos.load(Ordering::Acquire)),
        })
    }

    fn importance(&self) -> f32 {
        f32::from_bits(self.importance_bits.load(Ordering::Acquire))
    }

    /// Multiply importance by `factor`, clamped to `[0, 1]`. Lock-free CAS
    /// loop; safe against concurrent decays and consolidations.
    fn scale_importance(&self, factor: f32) {
        let mut current = self.importance_bits.load(Ordering::Acquire);
        loop {
            let updated = (f32::from_bits(current) * factor).clamp(0.0, 1.0);
            match self.importance_bits.compare_exchange_weak(
                current,
                updated.to_bits(),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    fn last_access(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_nanos(self.last_access_nanos.load(Ordering::Acquire))
    }

    fn record_access(&self) {
        self.access_count.fetch_add(1, Ordering::AcqRel);
        self.last_access_nanos
            .store(nanos_since_epoch(SystemTime::now()), Ordering::Release);
    }

    fn materialize(&self) -> Memory {
        Memory {
            vector: Vector {
                id: self.id.clone(),
                data: self.data.clone(),
            },
            attributes: MemoryAttributes {
                timestamp: self.timestamp,
                importance: self.importance(),
                context: self.context.clone(),
                decay_rate: self.decay_rate,
                relationships: self.relationships.to_vec(),
                access_count: self.access_count.load(Ordering::Acquire),
                last_access: self.last_access(),
            },
        }
    }
}

/// A temporal vector store, shareable across threads (`&self` API).
///
/// See the [crate-level documentation](crate) for an end-to-end example.
pub struct ChronoMind {
    config: Config,
    metric: Arc<dyn DistanceMetric>,
    index: LockFreeHnsw,
    by_id: papaya::HashMap<String, Arc<StoredMemory>>,
    by_handle: papaya::HashMap<u32, Arc<StoredMemory>>,
}

impl std::fmt::Debug for ChronoMind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChronoMind")
            .field("config", &self.config)
            .field("metric", &self.metric.name())
            .field("memories", &self.len())
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
        let index = LockFreeHnsw::new(config.index.clone(), Arc::clone(&metric));
        Ok(Self {
            config,
            metric,
            index,
            by_id: papaya::HashMap::new(),
            by_handle: papaya::HashMap::new(),
        })
    }

    /// The store's configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Number of stored memories.
    pub fn len(&self) -> usize {
        self.by_id.pin().len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.pin().is_empty()
    }

    /// Insert a memory, replacing any existing memory with the same id.
    ///
    /// When replacing, relationship links from the previous memory are
    /// merged into the new one (deduplicated, capped at
    /// [`max_relationships`](Config::max_relationships)).
    ///
    /// Concurrency: inserts from multiple threads are lock-free. The
    /// capacity check is approximate under concurrency — simultaneous
    /// inserts may overshoot `max_memories` by at most the number of
    /// concurrently inserting threads.
    #[instrument(skip(self, memory), fields(id = %memory.vector.id))]
    pub fn insert(&self, mut memory: Memory) -> Result<()> {
        memory.validate(&self.config)?;

        let map = self.by_id.pin();
        if let Some(existing) = map.get(&memory.vector.id) {
            let mut links: Vec<String> = existing.relationships.to_vec();
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
        } else if map.len() >= self.config.max_memories {
            return Err(Error::CapacityExceeded(self.config.max_memories));
        } else {
            memory
                .attributes
                .relationships
                .truncate(self.config.max_relationships);
        }

        let handle = self.index.insert(&memory.vector.data);
        let stored = StoredMemory::from_memory(&memory, handle);
        self.by_handle.pin().insert(handle, Arc::clone(&stored));
        if let Some(replaced) = map.insert(memory.vector.id.clone(), stored) {
            // The old record loses both its index node and its handle entry.
            self.index.remove(replaced.handle);
            self.by_handle.pin().remove(&replaced.handle);
        }
        Ok(())
    }

    /// Get a memory by id.
    pub fn get(&self, id: &str) -> Option<Memory> {
        self.by_id.pin().get(id).map(|s| s.materialize())
    }

    /// Get a memory by id, recording the access (bumps
    /// [`access_count`](crate::MemoryAttributes::access_count) and
    /// refreshes [`last_access`](crate::MemoryAttributes::last_access)).
    pub fn access(&self, id: &str) -> Option<Memory> {
        let map = self.by_id.pin();
        let stored = map.get(id)?;
        stored.record_access();
        Some(stored.materialize())
    }

    /// Remove a memory by id, returning it if present.
    pub fn remove(&self, id: &str) -> Option<Memory> {
        let removed = self.by_id.pin().remove(id).map(|s| {
            self.index.remove(s.handle);
            self.by_handle.pin().remove(&s.handle);
            s.materialize()
        });
        removed
    }

    /// A point-in-time snapshot of all stored memories, in arbitrary order.
    ///
    /// Concurrent writers may add or remove entries while the snapshot is
    /// being taken; the result is a consistent weak snapshot, not a frozen
    /// view.
    pub fn snapshot(&self) -> Vec<Memory> {
        self.by_id.pin().values().map(|s| s.materialize()).collect()
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
    ///
    /// Wait-free with respect to concurrent writers.
    #[instrument(skip(self, query))]
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(Memory, f32)>> {
        self.validate_query(query)?;
        let ef = self.config.index.ef_search.max(k * OVERSAMPLE);
        let now = SystemTime::now();
        let handles = self.by_handle.pin();

        let mut scored: Vec<(Memory, f32)> = self
            .index
            .search(query, ef)
            .into_iter()
            .filter_map(|(handle, distance)| {
                let stored = handles.get(&handle)?;
                let score = self.combined_score(distance, stored.timestamp, stored.decay_rate, now);
                Some((stored.materialize(), score))
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
            .by_id
            .pin()
            .values()
            .filter(|s| s.context == context)
            .map(|s| {
                let distance = self.metric.distance(&s.data, query);
                let score = self.combined_score(distance, s.timestamp, s.decay_rate, now);
                (s.materialize(), score)
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
    fn combined_score(
        &self,
        distance: f32,
        timestamp: SystemTime,
        decay_rate: f32,
        now: SystemTime,
    ) -> f32 {
        let w = self.config.temporal_weight;
        let age_hours = now
            .duration_since(timestamp)
            .unwrap_or_default()
            .as_secs_f32()
            / SECONDS_PER_HOUR;
        let rate = if decay_rate > 0.0 {
            decay_rate
        } else {
            self.config.base_decay_rate
        };
        let temporal_relevance = (-rate * age_hours).exp(); // 1 = fresh, 0 = ancient
        (1.0 - w) * (distance / 2.0) + w * (1.0 - temporal_relevance)
    }

    /// Decay every memory's importance based on time since last access.
    ///
    /// Importance is multiplied by `exp(-r * h)` where `r` is the memory's
    /// effective decay rate and `h` is hours since
    /// [`last_access`](crate::MemoryAttributes::last_access), then clamped
    /// to `[0.0, 1.0]`. Lock-free: each update is an atomic CAS loop, and
    /// the sweep can run concurrently with reads, writes, and other decays.
    #[instrument(skip(self))]
    pub fn apply_decay(&self) {
        let now = SystemTime::now();
        for stored in self.by_id.pin().values() {
            let hours = now
                .duration_since(stored.last_access())
                .unwrap_or_default()
                .as_secs_f32()
                / SECONDS_PER_HOUR;
            let rate = if stored.decay_rate > 0.0 {
                stored.decay_rate
            } else {
                self.config.base_decay_rate
            };
            stored.scale_importance((-rate * hours).exp());
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
    /// Takes `&mut self`: this is an `O(n²)` maintenance pass that wants
    /// exclusive access for trivially correct pairwise bookkeeping — run it
    /// from a maintenance thread, not the hot path.
    #[instrument(skip(self))]
    pub fn consolidate(&mut self) -> usize {
        let records: Vec<Arc<StoredMemory>> = self.by_id.pin().values().cloned().collect();
        let mut absorbed: HashSet<String> = HashSet::new();

        for i in 0..records.len() {
            if absorbed.contains(&records[i].id) {
                continue;
            }
            for j in (i + 1)..records.len() {
                if absorbed.contains(&records[j].id) || absorbed.contains(&records[i].id) {
                    continue;
                }
                let (a, b) = (&records[i], &records[j]);
                let similarity = self.metric.similarity(&a.data, &b.data);
                if similarity <= self.config.similarity_threshold {
                    continue;
                }

                // Keep the more important memory; absorb the other.
                let (keeper, dropped) = if a.importance() >= b.importance() {
                    (a, b)
                } else {
                    (b, a)
                };

                let mut links: Vec<String> = keeper.relationships.to_vec();
                let known: HashSet<String> = links.iter().cloned().collect();
                for link in dropped.relationships.iter() {
                    if link != &keeper.id && !known.contains(link) {
                        links.push(link.clone());
                    }
                }
                links.truncate(self.config.max_relationships);
                let importance = keeper.importance().max(dropped.importance());

                let map = self.by_id.pin();
                let rebuilt = keeper.rebuilt(links, importance);
                self.by_handle
                    .pin()
                    .insert(rebuilt.handle, Arc::clone(&rebuilt));
                map.insert(keeper.id.clone(), rebuilt);

                map.remove(&dropped.id);
                self.by_handle.pin().remove(&dropped.handle);
                self.index.remove(dropped.handle);

                debug!(kept = %keeper.id, dropped = %dropped.id, similarity, "consolidated");
                absorbed.insert(dropped.id.clone());
            }
        }

        absorbed.len()
    }

    /// Memories reachable from `id` by following relationship links, up to
    /// `max_depth` hops, in breadth-first order. The starting memory is not
    /// included.
    pub fn related(&self, id: &str, max_depth: usize) -> Vec<Memory> {
        let map = self.by_id.pin();
        let mut visited: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();

        visited.insert(id.to_string());
        queue.push_back((id.to_string(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let Some(stored) = map.get(&current) else {
                continue;
            };
            for link in stored.relationships.iter() {
                if visited.insert(link.clone()) {
                    if let Some(linked) = map.get(link) {
                        result.push(linked.materialize());
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
        let map = self.by_id.pin();
        let mut count = 0usize;
        let mut centroid = vec![0.0f32; self.config.dimensions];
        let mut importance_sum = 0.0f32;

        for stored in map.values().filter(|s| s.context == context) {
            count += 1;
            importance_sum += stored.importance();
            for (acc, x) in centroid.iter_mut().zip(&stored.data) {
                *acc += x;
            }
        }
        if count == 0 {
            return None;
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
        let map = self.by_id.pin();
        let mut total = 0usize;
        let mut total_components = 0usize;
        let mut importance_sum = 0.0f32;
        let mut contexts: HashMap<String, usize> = HashMap::new();
        let mut references: HashMap<String, usize> = HashMap::new();

        for stored in map.values() {
            total += 1;
            total_components += stored.data.len();
            importance_sum += stored.importance();
            *contexts.entry(stored.context.clone()).or_insert(0) += 1;
            for link in stored.relationships.iter() {
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
