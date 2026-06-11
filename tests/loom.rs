//! Loom model checks for the copy-on-write neighbor-list CAS protocol.
//!
//! Run with:
//! ```text
//! RUSTFLAGS="--cfg loom" cargo test --test loom --release
//! ```
//!
//! What is verified here, exhaustively over all interleavings loom can
//! reach: the load → copy-modify → compare_exchange → retry protocol used
//! by `index::neighbors::NeighborList::update` admits **no lost updates**
//! and **no torn reads**, under the exact memory orderings production uses
//! (Acquire loads, AcqRel CAS).
//!
//! What is deliberately not modeled: epoch-based reclamation. The model
//! leaks replaced slices instead of freeing them, because reclamation
//! safety is crossbeam-epoch's verified responsibility; our obligation —
//! defer destruction strictly after a successful unlink, readers always
//! pinned — is enforced by the `NeighborList` API shape and exercised
//! under Miri and the multi-threaded smoke tests. See docs/DESIGN.md §5.
#![cfg(loom)]

use loom::sync::atomic::{AtomicPtr, Ordering};
use loom::sync::Arc;
use loom::thread;

/// Model of `NeighborList`: same pointer discipline and memory orderings,
/// leak-based reclamation (loom models are tiny; leaks are fine and keep
/// every historical snapshot dereferenceable so torn reads would be
/// caught rather than masked by a crash).
struct ModelList {
    head: AtomicPtr<Vec<u32>>,
}

impl ModelList {
    fn new() -> Self {
        Self {
            head: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// Mirror of `NeighborList::load` (wait-free read path).
    fn load(&self) -> &[u32] {
        let ptr = self.head.load(Ordering::Acquire);
        if ptr.is_null() {
            &[]
        } else {
            // SAFETY: published pointers are never freed in the model.
            unsafe { &*ptr }
        }
    }

    /// Mirror of `NeighborList::update` (lock-free CAS-retry write path).
    fn update(&self, f: impl Fn(&[u32]) -> Vec<u32>) {
        loop {
            let current = self.head.load(Ordering::Acquire);
            let current_ids: &[u32] = if current.is_null() {
                &[]
            } else {
                // SAFETY: published pointers are never freed in the model.
                unsafe { &*current }
            };

            let new = Box::into_raw(Box::new(f(current_ids)));
            match self
                .head
                .compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return, // `current` is leaked: model-only behavior
                Err(_) => {
                    // SAFETY: `new` was never published; reclaim and retry.
                    unsafe { drop(Box::from_raw(new)) };
                }
            }
        }
    }
}

/// Two writers CAS-appending distinct values: every interleaving must end
/// with both values present. A lost update (one writer's effect silently
/// overwritten) fails this for some interleaving.
#[test]
fn concurrent_appends_are_never_lost() {
    loom::model(|| {
        let list = Arc::new(ModelList::new());

        let writers: Vec<_> = [1u32, 2u32]
            .into_iter()
            .map(|value| {
                let list = Arc::clone(&list);
                thread::spawn(move || {
                    list.update(|ids| {
                        let mut v = ids.to_vec();
                        v.push(value);
                        v
                    });
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }

        let mut final_ids = list.load().to_vec();
        final_ids.sort_unstable();
        assert_eq!(final_ids, vec![1, 2], "an update was lost");
    });
}

/// A reader racing a writer must observe either the old or the new list —
/// fully formed in both cases. Catches insufficient publish ordering
/// (e.g. a Relaxed store would let the reader see the pointer before the
/// Vec's contents).
#[test]
fn reader_sees_old_or_new_never_torn() {
    loom::model(|| {
        let list = Arc::new(ModelList::new());
        list.update(|_| vec![10]);

        let writer = {
            let list = Arc::clone(&list);
            thread::spawn(move || {
                list.update(|ids| {
                    let mut v = ids.to_vec();
                    v.push(20);
                    v
                });
            })
        };

        let seen = list.load().to_vec();
        assert!(
            seen == vec![10] || seen == vec![10, 20],
            "torn or impossible read: {seen:?}"
        );

        writer.join().unwrap();
        assert_eq!(list.load(), &[10, 20]);
    });
}

/// Three writers, two of which target the same logical change ("ensure 7
/// is present"): idempotent intents must compose with a competing append
/// without duplication or loss.
#[test]
fn idempotent_intents_compose() {
    loom::model(|| {
        let list = Arc::new(ModelList::new());

        let ensure_seven = || {
            let list = Arc::clone(&list);
            thread::spawn(move || {
                list.update(|ids| {
                    if ids.contains(&7) {
                        ids.to_vec() // no-op rewrite; preserves contents
                    } else {
                        let mut v = ids.to_vec();
                        v.push(7);
                        v
                    }
                });
            })
        };
        let a = ensure_seven();
        let b = ensure_seven();
        let c = {
            let list = Arc::clone(&list);
            thread::spawn(move || {
                list.update(|ids| {
                    let mut v = ids.to_vec();
                    v.push(9);
                    v
                });
            })
        };
        a.join().unwrap();
        b.join().unwrap();
        c.join().unwrap();

        let mut final_ids = list.load().to_vec();
        final_ids.sort_unstable();
        assert_eq!(final_ids, vec![7, 9], "intents did not compose");
    });
}
