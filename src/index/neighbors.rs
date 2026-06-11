//! Copy-on-write neighbor lists with epoch-based reclamation.
//!
//! This is the concurrency primitive at the heart of the lock-free index.
//! Each (node, layer) pair owns a [`NeighborList`]: an atomic pointer to an
//! immutable slice of neighbor ids.
//!
//! - **Readers are wait-free**: pin the epoch, load the pointer, iterate.
//!   No CAS, no retries, no locks; a reader can never be blocked or
//!   restarted by writers.
//! - **Writers are lock-free**: build a modified copy of the slice and
//!   publish it with a single CAS, retrying from the fresh value on
//!   contention. Some writer always makes progress; an individual writer
//!   only retries when another writer succeeded.
//! - **Reclamation is epoch-based** (`crossbeam-epoch`): a replaced slice
//!   is destroyed only after every thread that could have observed it has
//!   unpinned, so readers never dereference freed memory.
//!
//! The CAS protocol (load → copy-modify → compare_exchange → retry) is
//! verified under loom in `tests/loom.rs` against lost-update and
//! torn-read failures; see `docs/DESIGN.md` §5 for what loom does and does
//! not cover.

use crossbeam_epoch::{Atomic, Guard, Owned, Shared};
use std::sync::atomic::Ordering;

/// An immutable snapshot of a neighbor list. Never mutated after publish.
struct Slice {
    ids: Box<[u32]>,
}

/// An atomically replaceable, immutable list of neighbor ids.
pub(crate) struct NeighborList {
    head: Atomic<Slice>,
}

impl NeighborList {
    /// An empty list.
    pub(crate) fn new() -> Self {
        Self {
            head: Atomic::null(),
        }
    }

    /// Read the current ids. Wait-free.
    ///
    /// The returned slice borrows the guard: it stays valid for as long as
    /// the epoch is pinned, even if writers replace the list concurrently.
    pub(crate) fn load<'g>(&self, guard: &'g Guard) -> &'g [u32] {
        let shared = self.head.load(Ordering::Acquire, guard);
        // SAFETY: a non-null pointer in `head` always references a `Slice`
        // published by `update`/`store`; epoch pinning (the guard) keeps it
        // alive until at least the guard drops.
        match unsafe { shared.as_ref() } {
            Some(slice) => &slice.ids,
            None => &[],
        }
    }

    /// Unconditionally replace the list.
    ///
    /// Last-write-wins: concurrent `update`s may be overwritten. Use only
    /// where exclusivity is structural (e.g. initializing a node before
    /// its handle is published to the graph).
    pub(crate) fn store(&self, ids: Vec<u32>, guard: &Guard) {
        let new = Owned::new(Slice { ids: ids.into() });
        let old = self.head.swap(new, Ordering::AcqRel, guard);
        if !old.is_null() {
            // SAFETY: `old` was just unlinked and can no longer be loaded
            // by new readers; epoch GC destroys it after current readers
            // unpin.
            unsafe { guard.defer_destroy(old) };
        }
    }

    /// Atomically transform the list with `f` under a CAS-retry loop.
    /// Lock-free.
    ///
    /// `f` receives the current ids and returns the replacement, or `None`
    /// to leave the list unchanged (the loop exits without writing).
    /// On CAS failure `f` runs again over the fresh value — it must be
    /// idempotent in intent (e.g. "ensure id X is present"), not effectful.
    pub(crate) fn update(&self, guard: &Guard, mut f: impl FnMut(&[u32]) -> Option<Vec<u32>>) {
        loop {
            let current = self.head.load(Ordering::Acquire, guard);
            // SAFETY: as in `load` — pinned epoch keeps `current` alive.
            let current_ids: &[u32] = match unsafe { current.as_ref() } {
                Some(slice) => &slice.ids,
                None => &[],
            };

            let Some(new_ids) = f(current_ids) else {
                return; // no change requested
            };

            let new = Owned::new(Slice {
                ids: new_ids.into(),
            });
            match self.head.compare_exchange(
                current,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
                guard,
            ) {
                Ok(_) => {
                    if !current.is_null() {
                        // SAFETY: `current` was atomically unlinked by this
                        // exact CAS; defer destruction past live readers.
                        unsafe { guard.defer_destroy(current) };
                    }
                    return;
                }
                Err(_) => {
                    // Another writer landed first; rebuild from the fresh
                    // value. The failed `Owned` is dropped here — it was
                    // never published.
                    continue;
                }
            }
        }
    }
}

impl Drop for NeighborList {
    fn drop(&mut self) {
        // SAFETY: `&mut self` proves no other thread can access this list,
        // and any reader that ever loaded the current slice has unpinned
        // (the owner of the list outlives its readers by construction —
        // lists live in the arena, which drops only with exclusive access).
        let shared: Shared<'_, Slice> = self
            .head
            .load(Ordering::Relaxed, unsafe { crossbeam_epoch::unprotected() });
        if !shared.is_null() {
            // SAFETY: see above; we own the last reference.
            drop(unsafe { shared.into_owned() });
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn empty_list_loads_empty() {
        let list = NeighborList::new();
        let guard = crossbeam_epoch::pin();
        assert!(list.load(&guard).is_empty());
    }

    #[test]
    fn store_replaces_contents() {
        let list = NeighborList::new();
        let guard = crossbeam_epoch::pin();
        list.store(vec![1, 2, 3], &guard);
        assert_eq!(list.load(&guard), &[1, 2, 3]);
        list.store(vec![9], &guard);
        assert_eq!(list.load(&guard), &[9]);
    }

    #[test]
    fn update_transforms_and_none_is_a_noop() {
        let list = NeighborList::new();
        let guard = crossbeam_epoch::pin();
        list.update(&guard, |ids| {
            let mut v = ids.to_vec();
            v.push(7);
            Some(v)
        });
        assert_eq!(list.load(&guard), &[7]);
        list.update(&guard, |_| None);
        assert_eq!(list.load(&guard), &[7]);
    }

    #[test]
    fn a_loaded_slice_survives_concurrent_replacement() {
        let list = NeighborList::new();
        let guard = crossbeam_epoch::pin();
        list.store(vec![1, 2, 3], &guard);

        let snapshot = list.load(&guard);
        list.store(vec![4], &guard); // replaces, defers destruction
                                     // The pinned snapshot still reads the old data safely.
        assert_eq!(snapshot, &[1, 2, 3]);
        assert_eq!(list.load(&guard), &[4]);
    }

    /// Lost-update smoke test: many threads CAS-appending unique values
    /// must all land. (Exhaustive small-scale verification of the same
    /// property lives in tests/loom.rs.)
    #[test]
    fn concurrent_updates_lose_nothing() {
        let list = Arc::new(NeighborList::new());
        // Miri interprets every instruction; keep its workload tiny.
        let threads = if cfg!(miri) { 3 } else { 8 };
        let per_thread = if cfg!(miri) { 8 } else { 100 };

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let list = Arc::clone(&list);
                std::thread::spawn(move || {
                    for i in 0..per_thread {
                        let value = t * per_thread + i;
                        let guard = crossbeam_epoch::pin();
                        list.update(&guard, |ids| {
                            let mut v = ids.to_vec();
                            v.push(value);
                            Some(v)
                        });
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        let guard = crossbeam_epoch::pin();
        let mut final_ids = list.load(&guard).to_vec();
        final_ids.sort_unstable();
        let expected: Vec<u32> = (0..threads * per_thread).collect();
        assert_eq!(final_ids, expected, "an update was lost");
    }
}
