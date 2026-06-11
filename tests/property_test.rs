//! Property tests for the index: invariants that must hold for arbitrary
//! inputs, not just hand-picked fixtures.

use std::sync::Arc;

use chronomind::config::IndexParams;
use chronomind::index::{RwLockHnsw, VectorIndex};
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
