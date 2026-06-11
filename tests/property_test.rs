//! Property tests for the index: invariants that must hold for arbitrary
//! inputs, not just hand-picked fixtures.

use std::sync::Arc;

use chronomind::config::IndexParams;
use chronomind::index::{LockFreeHnsw, RwLockHnsw, VectorIndex};
use chronomind::metric::{CosineDistance, DistanceMetric};

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
            index.insert(v).unwrap();
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
                0 => handles.push(index.insert(v).unwrap()),
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

    /// Differential oracle: in the exhaustive regime (`ef` covering every
    /// live vector), HNSW search is exact — so both index implementations,
    /// run through an arbitrary insert/remove sequence, must agree
    /// *perfectly* with a brute-force linear-scan model: same ids, same
    /// order, same distances. Catches lost inserts, ghost tombstones,
    /// distance corruption, and graph disconnection in either impl.
    #[test]
    fn both_indexes_match_the_linear_scan_model_exactly(
        ops in pvec((0u8..=1, arb_vector()), 1..48),
        queries in pvec(arb_vector(), 1..4),
    ) {
        let metric = CosineDistance::new();
        let params = IndexParams::default();
        let indexes: [Box<dyn VectorIndex>; 2] = [
            Box::new(LockFreeHnsw::with_seed(
                params.clone(), Arc::new(CosineDistance::new()), 31,
            )),
            Box::new(RwLockHnsw::with_seed(
                params, Arc::new(CosineDistance::new()), 37,
            )),
        ];
        // Model: handle = insertion order (true for both impls), bool = live.
        let mut model: Vec<(Vec<f32>, bool)> = Vec::new();

        for (i, (kind, v)) in ops.iter().enumerate() {
            match kind {
                0 => {
                    for index in &indexes {
                        prop_assert_eq!(
                            index.insert(v),
                            Some(model.len() as u32),
                            "handles must follow insertion order"
                        );
                    }
                    model.push((v.clone(), true));
                }
                _ => {
                    if !model.is_empty() {
                        let pick = i % model.len();
                        let expect_removed = model[pick].1;
                        for index in &indexes {
                            prop_assert_eq!(index.remove(pick as u32), expect_removed);
                        }
                        model[pick].1 = false;
                    }
                }
            }
        }

        let live: Vec<(u32, &Vec<f32>)> = model
            .iter()
            .enumerate()
            .filter(|(_, (_, alive))| *alive)
            .map(|(h, (v, _))| (h as u32, v))
            .collect();

        for q in &queries {
            let mut expected: Vec<(f32, u32)> = live
                .iter()
                .map(|(h, v)| (metric.distance(v, q), *h))
                .collect();
            expected.sort_by(|a, b| a.0.total_cmp(&b.0));

            for (which, index) in indexes.iter().enumerate() {
                let got = index.search(q, model.len() + 8); // exhaustive ef
                prop_assert_eq!(
                    got.len(), expected.len(),
                    "impl {} returned {} of {} live vectors",
                    which, got.len(), expected.len()
                );
                for ((got_id, got_dist), (want_dist, want_id)) in
                    got.iter().zip(&expected)
                {
                    prop_assert_eq!(
                        got_id, want_id,
                        "impl {} ranking diverged from linear scan", which
                    );
                    prop_assert!(
                        (got_dist - want_dist).abs() < 1e-6,
                        "impl {which} distance {got_dist} vs model {want_dist}"
                    );
                }
            }
        }
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
            index.insert(v).unwrap();
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
