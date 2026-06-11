//! A lock-free, chunked, append-only arena.
//!
//! Nodes are allocated by bumping an atomic counter and never move or get
//! freed while the arena lives — which is what makes wait-free reads
//! possible elsewhere: a `u32` handle, once published, dereferences to a
//! stable address with no reclamation protocol needed for the node itself.
//!
//! Memory is allocated in fixed-size chunks installed into a preallocated
//! pointer directory with a single CAS, so pushing never relocates
//! existing slots (unlike `Vec` growth). Each slot carries a `ready` flag
//! published with `Release` ordering: a handle obtained from any source is
//! safe to read exactly when the flag is observed `true` with `Acquire`.
//!
//! The module is `loom`-instrumented: under `--cfg loom` the atomics and
//! cells come from loom and the chunk geometry shrinks so loom can
//! exhaustively model-check the interleavings.

use std::mem::MaybeUninit;
use std::ptr;

#[cfg(loom)]
use loom::cell::UnsafeCell;
#[cfg(loom)]
use loom::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
#[cfg(not(loom))]
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

/// Shim matching `loom::cell::UnsafeCell`'s closure API over the std cell,
/// so the same code compiles under both cfgs.
#[cfg(not(loom))]
#[derive(Debug)]
struct UnsafeCell<T>(std::cell::UnsafeCell<T>);

#[cfg(not(loom))]
impl<T> UnsafeCell<T> {
    fn new(value: T) -> Self {
        Self(std::cell::UnsafeCell::new(value))
    }
    fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
        f(self.0.get())
    }
    fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
        f(self.0.get())
    }
}

#[cfg(not(loom))]
const CHUNK_SIZE: usize = 4096;
#[cfg(not(loom))]
const MAX_CHUNKS: usize = 4096; // 16.7M slots; directory costs 32 KiB

#[cfg(loom)]
const CHUNK_SIZE: usize = 2;
#[cfg(loom)]
const MAX_CHUNKS: usize = 4;

struct Slot<T> {
    /// `true` once `value` is fully written; `Release`-stored by the
    /// writer, `Acquire`-loaded by readers.
    ready: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
}

struct Chunk<T> {
    slots: Box<[Slot<T>]>,
}

impl<T> Chunk<T> {
    fn new() -> Self {
        Self {
            slots: (0..CHUNK_SIZE)
                .map(|_| Slot {
                    ready: AtomicBool::new(false),
                    value: UnsafeCell::new(MaybeUninit::uninit()),
                })
                .collect(),
        }
    }
}

/// The arena. See the module docs.
pub struct Arena<T> {
    /// Directory of lazily allocated chunks.
    chunks: Box<[AtomicPtr<Chunk<T>>]>,
    /// Number of slots ever reserved (a bound, not a publication marker —
    /// the per-slot `ready` flag is what publishes data).
    reserved: AtomicUsize,
}

// SAFETY: `Arena` hands out `&T` to multiple threads and accepts pushes
// from multiple threads. Values are written exactly once by the thread
// that reserved the slot (exclusive by fetch_add), published via the
// `ready` flag's Release/Acquire pair, and never moved or dropped until
// the arena itself drops with exclusive access.
unsafe impl<T: Send + Sync> Send for Arena<T> {}
// SAFETY: see above.
unsafe impl<T: Send + Sync> Sync for Arena<T> {}

impl<T> Arena<T> {
    /// Total slot capacity.
    pub const CAPACITY: usize = CHUNK_SIZE * MAX_CHUNKS;

    pub fn new() -> Self {
        Self {
            chunks: (0..MAX_CHUNKS).map(|_| AtomicPtr::default()).collect(),
            reserved: AtomicUsize::new(0),
        }
    }

    /// Append `value`, returning its stable handle. Lock-free: one
    /// `fetch_add` plus at most one chunk-installation CAS race.
    ///
    /// # Panics
    /// If the arena is at [`CAPACITY`](Self::CAPACITY).
    pub fn push(&self, value: T) -> u32 {
        let index = self.reserved.fetch_add(1, Ordering::Relaxed);
        assert!(
            index < Self::CAPACITY,
            "arena is full ({} slots)",
            Self::CAPACITY
        );

        let chunk = self.chunk_or_install(index / CHUNK_SIZE);
        let slot = &chunk.slots[index % CHUNK_SIZE];

        // SAFETY: `fetch_add` reserved `index` for this thread exclusively;
        // no other thread writes this slot, and no reader dereferences the
        // value until `ready` is observed `true`.
        slot.value.with_mut(|p| unsafe { (*p).write(value) });
        slot.ready.store(true, Ordering::Release);
        index as u32
    }

    /// Read the value at `handle`, or `None` if the handle was never
    /// returned by [`push`](Self::push) or its write has not been published
    /// to this thread yet. Wait-free.
    pub fn get(&self, handle: u32) -> Option<&T> {
        let index = handle as usize;
        if index >= Self::CAPACITY {
            return None;
        }
        let chunk_ptr = self.chunks[index / CHUNK_SIZE].load(Ordering::Acquire);
        if chunk_ptr.is_null() {
            return None;
        }
        // SAFETY: a non-null chunk pointer is only ever installed by
        // `chunk_or_install` (Release CAS) and never replaced or freed
        // while the arena lives.
        let slot = unsafe { &(*chunk_ptr).slots[index % CHUNK_SIZE] };
        if !slot.ready.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: `ready == true` (Acquire) happens-after the writer's full
        // initialization (Release); the value is immutable from then on.
        Some(slot.value.with(|p| unsafe { (*p).assume_init_ref() }))
    }

    /// Number of slots reserved so far. Handles `0..len()` may still be
    /// momentarily unpublished; [`get`](Self::get) is the authority.
    pub fn len(&self) -> usize {
        self.reserved.load(Ordering::Acquire).min(Self::CAPACITY)
    }

    /// Whether no slot has ever been reserved.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn chunk_or_install(&self, chunk_index: usize) -> &Chunk<T> {
        let slot = &self.chunks[chunk_index];
        let existing = slot.load(Ordering::Acquire);
        if !existing.is_null() {
            // SAFETY: installed chunks are never freed while the arena lives.
            return unsafe { &*existing };
        }

        let fresh = Box::into_raw(Box::new(Chunk::<T>::new()));
        match slot.compare_exchange(ptr::null_mut(), fresh, Ordering::AcqRel, Ordering::Acquire) {
            // SAFETY: we just installed `fresh`; it stays alive with the arena.
            Ok(_) => unsafe { &*fresh },
            Err(winner) => {
                // Another thread won the install race; discard ours.
                // SAFETY: `fresh` was never shared.
                unsafe { drop(Box::from_raw(fresh)) };
                // SAFETY: the winning pointer is non-null and arena-owned.
                unsafe { &*winner }
            }
        }
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Arena<T> {
    fn drop(&mut self) {
        for chunk_slot in self.chunks.iter() {
            let chunk_ptr = chunk_slot.load(Ordering::Acquire);
            if chunk_ptr.is_null() {
                continue;
            }
            // SAFETY: `&mut self` gives exclusive access; the chunk was
            // installed by us and never freed before now.
            let chunk = unsafe { Box::from_raw(chunk_ptr) };
            for slot in chunk.slots.iter() {
                if slot.ready.load(Ordering::Acquire) {
                    // SAFETY: ready slots hold initialized values that were
                    // never dropped; we have exclusive access.
                    slot.value.with_mut(|p| unsafe { (*p).assume_init_drop() });
                }
            }
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn push_then_get_roundtrips() {
        let arena: Arena<String> = Arena::new();
        let a = arena.push("alpha".into());
        let b = arena.push("beta".into());
        assert_eq!(arena.get(a).unwrap(), "alpha");
        assert_eq!(arena.get(b).unwrap(), "beta");
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn unknown_handles_return_none() {
        let arena: Arena<u64> = Arena::new();
        assert!(arena.get(0).is_none());
        assert!(arena.get(u32::MAX).is_none());
    }

    #[test]
    fn handles_cross_chunk_boundaries() {
        let arena: Arena<usize> = Arena::new();
        let n = if cfg!(miri) {
            CHUNK_SIZE + 3 // one boundary is enough under interpretation
        } else {
            CHUNK_SIZE * 2 + 3
        };
        for i in 0..n {
            assert_eq!(arena.push(i), i as u32);
        }
        for i in 0..n {
            assert_eq!(*arena.get(i as u32).unwrap(), i);
        }
    }

    #[test]
    fn values_are_dropped_with_the_arena() {
        struct Counted(Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Counted {
            fn drop(&mut self) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let arena: Arena<Counted> = Arena::new();
        for _ in 0..10 {
            arena.push(Counted(Arc::clone(&drops)));
        }
        drop(arena);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 10);
    }

    #[test]
    fn concurrent_pushes_get_distinct_readable_slots() {
        let arena: Arc<Arena<(usize, usize)>> = Arc::new(Arena::new());
        // Miri interprets every instruction; keep its workload tiny while
        // still crossing a chunk boundary.
        let threads = if cfg!(miri) { 4 } else { 8 };
        let per_thread = if cfg!(miri) { 16 } else { 1000 };

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let arena = Arc::clone(&arena);
                std::thread::spawn(move || {
                    (0..per_thread)
                        .map(|i| (arena.push((t, i)), (t, i)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        for handle in handles {
            for (id, expected) in handle.join().unwrap() {
                assert!(seen.insert(id), "duplicate handle {id}");
                assert_eq!(*arena.get(id).unwrap(), expected);
            }
        }
        assert_eq!(seen.len(), threads * per_thread);
    }
}

// Loom model checks: exhaustive interleaving verification of the
// reserve/write/publish protocol. Run with:
//   RUSTFLAGS="--cfg loom" cargo test --lib --release loom_
#[cfg(all(test, loom))]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::thread;

    #[test]
    fn loom_concurrent_pushes_are_distinct_and_readable() {
        loom::model(|| {
            let arena: Arc<Arena<usize>> = Arc::new(Arena::new());

            let a = {
                let arena = Arc::clone(&arena);
                thread::spawn(move || arena.push(1))
            };
            let b = {
                let arena = Arc::clone(&arena);
                thread::spawn(move || arena.push(2))
            };
            let ha = a.join().unwrap();
            let hb = b.join().unwrap();

            assert_ne!(ha, hb, "two pushes must reserve distinct slots");
            assert_eq!(*arena.get(ha).unwrap(), 1);
            assert_eq!(*arena.get(hb).unwrap(), 2);
        });
    }

    #[test]
    fn loom_reader_sees_none_or_initialized_value() {
        loom::model(|| {
            let arena: Arc<Arena<u64>> = Arc::new(Arena::new());

            let writer = {
                let arena = Arc::clone(&arena);
                thread::spawn(move || {
                    arena.push(0xFEED);
                })
            };
            // Race a read against the push: it must observe either absence
            // or the fully initialized value — never a torn slot.
            match arena.get(0) {
                None => {}
                Some(&v) => assert_eq!(v, 0xFEED),
            }
            writer.join().unwrap();
            assert_eq!(*arena.get(0).unwrap(), 0xFEED);
        });
    }

    #[test]
    fn loom_chunk_install_race_is_safe() {
        loom::model(|| {
            // CHUNK_SIZE is 2 under loom: two threads pushing two values
            // each forces a race on installing the second chunk.
            let arena: Arc<Arena<usize>> = Arc::new(Arena::new());
            arena.push(100); // occupy slot 0

            let a = {
                let arena = Arc::clone(&arena);
                thread::spawn(move || (arena.push(1), arena.push(2)))
            };
            let b = {
                let arena = Arc::clone(&arena);
                thread::spawn(move || (arena.push(3), arena.push(4)))
            };
            let (a1, a2) = a.join().unwrap();
            let (b1, b2) = b.join().unwrap();

            assert_eq!(*arena.get(a1).unwrap(), 1);
            assert_eq!(*arena.get(a2).unwrap(), 2);
            assert_eq!(*arena.get(b1).unwrap(), 3);
            assert_eq!(*arena.get(b2).unwrap(), 4);
        });
    }
}
