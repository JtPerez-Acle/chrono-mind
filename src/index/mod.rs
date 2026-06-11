//! Vector indexes: approximate nearest-neighbor search structures.
//!
//! Indexes here are purely geometric — they know nothing about temporal
//! attributes. Temporal relevance is applied by
//! [`ChronoMind`](crate::ChronoMind) as a rerank over the candidates an
//! index returns (see the design notes in `docs/DESIGN.md`).
//!
//! Ids are dense `u32` handles assigned by the index at insert time; the
//! store maintains the mapping between caller-facing string ids and index
//! handles.

// The primitives are public-but-hidden: not part of the stable API, but
// reachable by the reclamation tests and fuzz targets, which need to drive
// them directly.
#[doc(hidden)]
pub mod arena;
mod lockfree_hnsw;
#[doc(hidden)]
pub mod neighbors;
mod rwlock_hnsw;
mod sharded_rwlock;

pub use lockfree_hnsw::LockFreeHnsw;
pub use rwlock_hnsw::RwLockHnsw;
pub use sharded_rwlock::ShardedRwLockHnsw;

/// Total slot capacity of the lock-free index's arena (handles are never
/// reused; tombstones count).
pub fn arena_capacity() -> usize {
    arena::Arena::<()>::CAPACITY
}

/// An `f32` wrapper with total ordering via [`f32::total_cmp`].
///
/// Heap orderings over raw `f32` break down in the presence of NaN; every
/// ordered collection in the index goes through this newtype instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TotalF32(pub f32);

impl Eq for TotalF32 {}

impl PartialOrd for TotalF32 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TotalF32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// A geometric nearest-neighbor index over `f32` vectors.
///
/// Implementations must be safe to share across threads. Distances are as
/// computed by the metric the index was constructed with (cosine by
/// default: `[0.0, 2.0]`, lower is closer).
pub trait VectorIndex: Send + Sync {
    /// Insert a vector, returning its dense handle, or `None` if the
    /// index's storage is exhausted (handles are never reused, so deleted
    /// entries count against capacity until a snapshot reload compacts).
    fn insert(&self, vector: &[f32]) -> Option<u32>;

    /// Mark a handle as deleted. Tombstoned entries stop appearing in
    /// search results but still route graph traversal. Returns `false` if
    /// the handle was unknown or already deleted.
    fn remove(&self, id: u32) -> bool;

    /// Return up to `ef` candidates nearest to `query` as
    /// `(handle, distance)`, sorted by ascending distance.
    fn search(&self, query: &[f32], ef: usize) -> Vec<(u32, f32)>;

    /// Number of live (non-tombstoned) vectors.
    fn len(&self) -> usize;

    /// Whether the index holds no live vectors.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
