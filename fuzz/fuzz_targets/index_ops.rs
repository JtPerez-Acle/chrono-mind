//! Coverage-guided fuzzing of the lock-free index with the structural
//! invariant sweep as the oracle.
//!
//! libFuzzer explores op sequences (insert / remove / search) the
//! hand-written stress tests never would; after every sequence the full
//! graph invariant checker must pass. Run locally with:
//!
//! ```text
//! cargo +nightly fuzz run index_ops -- -max_total_time=300
//! ```
#![no_main]

use std::sync::Arc;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, VectorIndex};
use chronomind::metric::CosineDistance;

const DIM: usize = 6;

#[derive(Arbitrary, Debug)]
enum Op {
    Insert([i8; DIM]),
    Remove(u8),
    Search([i8; DIM], u8),
}

fn to_vector(raw: &[i8; DIM]) -> Option<Vec<f32>> {
    let v: Vec<f32> = raw.iter().map(|&x| x as f32 / 16.0).collect();
    // The index contract excludes degenerate vectors; the store layer
    // validates them away before they ever reach it.
    if v.iter().map(|x| x * x).sum::<f32>() < 1e-6 {
        None
    } else {
        Some(v)
    }
}

fuzz_target!(|ops: Vec<Op>| {
    if ops.len() > 256 {
        return; // keep individual cases fast; coverage drives exploration
    }

    let index = LockFreeHnsw::with_seed(
        IndexParams {
            max_connections: 4,
            ef_construction: 16,
            ef_search: 16,
        },
        Arc::new(CosineDistance::new()),
        0xF022,
    );
    let mut handles: Vec<u32> = Vec::new();

    for op in &ops {
        match op {
            Op::Insert(raw) => {
                if let Some(v) = to_vector(raw) {
                    handles.push(index.insert(&v));
                }
            }
            Op::Remove(pick) => {
                if !handles.is_empty() {
                    index.remove(handles[*pick as usize % handles.len()]);
                }
            }
            Op::Search(raw, ef) => {
                if let Some(v) = to_vector(raw) {
                    let results = index.search(&v, (*ef as usize % 32).max(1));
                    assert!(
                        results.windows(2).all(|w| w[0].1 <= w[1].1),
                        "unsorted results"
                    );
                }
            }
        }
    }

    index.check_invariants().expect("graph invariants violated");
});
