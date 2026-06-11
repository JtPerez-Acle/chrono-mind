//! Reclamation gates: epoch-based schemes can be correct and still never
//! free anything. This binary installs a counting global allocator and
//! verifies that the COW neighbor-list garbage actually gets reclaimed —
//! and demonstrates the known pathology (a guard pinned forever blocks
//! reclamation) honestly rather than hiding it.
//!
//! Both scenarios live in ONE #[test] so no parallel test skews the
//! global allocation counters.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use chronomind::index::neighbors::NeighborList;

struct CountingAllocator;

static NET_BYTES: AtomicIsize = AtomicIsize::new(0);

// SAFETY: defers entirely to the system allocator; only bookkeeping added.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            NET_BYTES.fetch_add(layout.size() as isize, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        NET_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            NET_BYTES.fetch_add(
                new_size as isize - layout.size() as isize,
                Ordering::Relaxed,
            );
        }
        p
    }
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

fn net_bytes() -> isize {
    NET_BYTES.load(Ordering::Relaxed)
}

/// Churn `updates`-per-thread COW replacements across `threads` threads.
fn churn(list: &Arc<NeighborList>, threads: u32, updates: u32) {
    std::thread::scope(|s| {
        for t in 0..threads {
            let list = Arc::clone(list);
            s.spawn(move || {
                for i in 0..updates {
                    let guard = crossbeam_epoch::pin();
                    list.update(&guard, |ids| {
                        // Fixed-size replacement: every update retires the
                        // previous 32-id slice into the epoch queue.
                        let mut v = Vec::with_capacity(32);
                        v.extend_from_slice(&ids[..ids.len().min(31)]);
                        v.push(t * updates + i);
                        Some(v)
                    });
                }
            });
        }
    });
}

/// Drive epoch advancement so deferred garbage gets collected.
fn flush_epochs() {
    for _ in 0..256 {
        crossbeam_epoch::pin().flush();
    }
}

const MB: isize = 1024 * 1024;

#[test]
fn epoch_reclamation_plateaus_and_pinned_guard_is_the_known_pathology() {
    // ---------- Scenario A: garbage is actually reclaimed ----------
    // 4 threads x 250k updates = 1M retired slices (~32 MB of garbage if
    // nothing were freed). Memory in flight must stay bounded by the
    // epoch collection cadence, nowhere near the total churned volume.
    let baseline = net_bytes();
    let list = Arc::new(NeighborList::new());

    churn(&list, 4, 250_000);
    let after_churn = net_bytes() - baseline;
    assert!(
        after_churn < 16 * MB,
        "in-flight garbage after 1M churned slices is {after_churn} bytes; \
         reclamation is not keeping up"
    );

    flush_epochs();
    drop(list);
    flush_epochs();
    let after_drop = net_bytes() - baseline;
    assert!(
        after_drop < MB,
        "{after_drop} bytes still live after drop + epoch flushes; leaked"
    );

    // ---------- Scenario B: the documented pathology ----------
    // One guard pinned forever blocks the epoch from advancing past it:
    // garbage accumulates without bound while it is held. This is the
    // known cost of epoch-based reclamation — verified here so the docs
    // can state it as measured behavior, not theory.
    let base2 = net_bytes();
    let list = Arc::new(NeighborList::new());
    let hostage_guard = crossbeam_epoch::pin();
    // Keep a snapshot alive so the pin is semantically load-bearing.
    let snapshot = list.load(&hostage_guard);

    churn(&list, 4, 100_000); // 400k retirements while pinned
    let pinned_growth = net_bytes() - base2;
    assert!(
        pinned_growth > 4 * MB,
        "expected unbounded garbage growth under a pinned guard, saw only \
         {pinned_growth} bytes — the pathology test is not exercising the epoch"
    );
    assert!(
        snapshot.is_empty(),
        "pre-churn snapshot must remain readable"
    );

    // Release the hostage: reclamation resumes. Collection is incremental
    // (a few bags per pin), so drive it with real traffic, then require the
    // in-flight volume to return to the same steady-state bound scenario A
    // established — not merely shrink.
    drop(hostage_guard);
    churn(&list, 4, 50_000);
    flush_epochs();
    let after_release = net_bytes() - base2;
    assert!(
        after_release < 16 * MB,
        "garbage did not drain after releasing the pinned guard: \
         {after_release} bytes in flight (was {pinned_growth} while pinned)"
    );
}
