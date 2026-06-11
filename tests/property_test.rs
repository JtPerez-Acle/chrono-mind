//! Property tests for the index: invariants that must hold for arbitrary
//! inputs, not just hand-picked fixtures.

use std::sync::Arc;

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, RwLockHnsw, VectorIndex};
use chronomind::metric::CosineDistance;

use proptest::collection::vec as pvec;
use proptest::prelude::*;

const DIM: usize = 8;

/// A non-degenerate vector: finite components, not the zero vector.
fn arb_vector() -> impl Strategy<Value = Vec<f32>> {
    pvec(-100.0f32..100.0, DIM).prop_filter("zero vector has no direction", |v| {
        v.iter().map(|x| x * x).sum::<f32>().sqrt() > 1e-3
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Searching with an inserted vector as the query must return distance
    /// ~0 as the best hit: every vector is its own nearest neighbor (up to
    /// direction duplicates, which also sit at distance 0).
    #[test]
    fn every_vector_finds_itself(vectors in pvec(arb_vector(), 1..50)) {
        let index = RwLockHnsw::with_seed(
            IndexParams::default(),
            Arc::new(CosineDistance::new()),
            7,
        );
        for v in &vectors {
            index.insert(v);
        }
        for v in &vectors {
            let results = index.search(v, 10);
            prop_assert!(!results.is_empty());
            prop_assert!(
                results[0].1 < 1e-5,
                "best distance {} for self-query",
                results[0].1
            );
        }
    }

    /// Arbitrary interleaved op sequences preserve every structural graph
    /// invariant — the invariant sweep is the oracle, the op sequence is
    /// the fuzz input. (The coverage-guided version of this same harness
    /// lives in fuzz/fuzz_targets/index_ops.rs.)
    #[test]
    fn arbitrary_op_sequences_preserve_index_invariants(
        ops in pvec((0u8..=2, arb_vector()), 1..120)
    ) {
        let index = LockFreeHnsw::with_seed(
            IndexParams::default(),
            Arc::new(CosineDistance::new()),
            23,
        );
        let mut handles: Vec<u32> = Vec::new();
        let mut removed = 0usize;

        for (i, (kind, v)) in ops.iter().enumerate() {
            match kind {
                0 => handles.push(index.insert(v)),
                1 => {
                    // Remove a deterministic-but-arbitrary previous insert.
                    if let Some(&h) = handles.get(i % handles.len().max(1)) {
                        if index.remove(h) {
                            removed += 1;
                        }
                    }
                }
                _ => {
                    let results = index.search(v, 16);
                    prop_assert!(
                        results.windows(2).all(|w| w[0].1 <= w[1].1),
                        "unsorted results mid-sequence"
                    );
                }
            }
        }

        prop_assert_eq!(index.len(), handles.len() - removed, "live count drifted");
        index.check_invariants().map_err(|e| {
            proptest::test_runner::TestCaseError::fail(format!("invariants: {e}"))
        })?;
    }

    /// Arbitrary op sequences against the full store API — including
    /// `consolidate` at quiesce points (single-threaded ownership here, the
    /// legal pattern) — keep the store coherent.
    #[test]
    fn arbitrary_op_sequences_keep_the_store_coherent(
        ops in pvec((0u8..=5, arb_vector()), 1..80)
    ) {
        let mut store = chronomind::ChronoMind::new(
            chronomind::Config::builder().dimensions(DIM).build().unwrap(),
        ).unwrap();
        let mut next_id = 0usize;

        for (i, (kind, v)) in ops.iter().enumerate() {
            match kind {
                0 | 1 => {
                    let memory = chronomind::Memory::from_vector(
                        chronomind::Vector::new(format!("m{next_id}"), v.clone()),
                    );
                    store.insert(memory).unwrap();
                    next_id += 1;
                }
                2 => {
                    if next_id > 0 {
                        store.remove(&format!("m{}", i % next_id));
                    }
                }
                3 => {
                    if next_id > 0 {
                        store.access(&format!("m{}", i % next_id));
                    }
                }
                4 => store.apply_decay(),
                _ => {
                    store.consolidate(); // quiesce point: we own the store
                }
            }
            let results = store.search(v, 5).unwrap();
            prop_assert!(results.len() <= 5);
            prop_assert!(
                results.windows(2).all(|w| w[0].1 <= w[1].1),
                "unsorted store results"
            );
        }

        let stats = store.stats();
        prop_assert_eq!(stats.total_memories, store.len(), "stats drifted from len");
    }

    /// Results are sorted ascending by distance and contain no duplicate ids.
    #[test]
    fn results_are_sorted_and_unique(vectors in pvec(arb_vector(), 1..50)) {
        let index = RwLockHnsw::with_seed(
            IndexParams::default(),
            Arc::new(CosineDistance::new()),
            11,
        );
        for v in &vectors {
            index.insert(v);
        }
        let query = &vectors[0];
        let results = index.search(query, 20);
        prop_assert!(results.windows(2).all(|w| w[0].1 <= w[1].1), "not sorted");
        let mut ids: Vec<u32> = results.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        prop_assert_eq!(ids.len(), results.len(), "duplicate ids in results");
    }
}
