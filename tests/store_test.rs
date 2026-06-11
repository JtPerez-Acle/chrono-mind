use std::time::{Duration, SystemTime};

use chronomind::{ChronoMind, Config, Error, Memory, MemoryAttributes, Vector};

fn config(dimensions: usize) -> Config {
    Config {
        dimensions,
        ..Config::default()
    }
}

fn memory(id: &str, data: Vec<f32>) -> Memory {
    Memory::from_vector(Vector::new(id, data))
}

fn memory_in_context(id: &str, data: Vec<f32>, context: &str) -> Memory {
    Memory::new(
        Vector::new(id, data),
        MemoryAttributes {
            context: context.into(),
            ..MemoryAttributes::default()
        },
    )
}

#[test]
fn insert_and_get_roundtrip() {
    let store = ChronoMind::new(config(3)).unwrap();
    store.insert(memory("a", vec![1.0, 0.0, 0.0])).unwrap();

    let got = store.get("a").unwrap();
    assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
    assert!(store.get("missing").is_none());
    assert_eq!(store.len(), 1);
}

#[test]
fn search_returns_nearest_first() {
    let store = ChronoMind::new(config(3)).unwrap();
    store.insert(memory("x", vec![1.0, 0.0, 0.0])).unwrap();
    store.insert(memory("y", vec![0.0, 1.0, 0.0])).unwrap();
    store.insert(memory("z", vec![0.7, 0.7, 0.0])).unwrap();

    let results = store.search(&[1.0, 0.1, 0.0], 3).unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0.vector.id, "x");
    assert_eq!(results[1].0.vector.id, "z");
    assert_eq!(results[2].0.vector.id, "y");
    // Scores ascend (lower = better).
    assert!(results[0].1 <= results[1].1 && results[1].1 <= results[2].1);
}

#[test]
fn search_k_truncates() {
    let store = ChronoMind::new(config(2)).unwrap();
    for i in 0..10 {
        let angle = i as f32 * 0.1;
        store
            .insert(memory(&format!("m{i}"), vec![angle.cos(), angle.sin()]))
            .unwrap();
    }
    assert_eq!(store.search(&[1.0, 0.0], 3).unwrap().len(), 3);
}

#[test]
fn temporal_weight_prefers_recent_memories() {
    // Two identical vectors; one is a week old. With temporal weighting the
    // fresh one must rank first.
    let store = ChronoMind::new(Config {
        dimensions: 2,
        temporal_weight: 0.5,
        ..Config::default()
    })
    .unwrap();

    let old_time = SystemTime::now() - Duration::from_secs(7 * 24 * 3600);
    store
        .insert(Memory::new(
            Vector::new("old", vec![1.0, 0.0]),
            MemoryAttributes {
                timestamp: old_time,
                last_access: old_time,
                ..MemoryAttributes::default()
            },
        ))
        .unwrap();
    store.insert(memory("fresh", vec![1.0, 0.0])).unwrap();

    let results = store.search(&[1.0, 0.0], 2).unwrap();
    assert_eq!(results[0].0.vector.id, "fresh");
    assert_eq!(results[1].0.vector.id, "old");
}

#[test]
fn zero_temporal_weight_ranks_purely_by_distance() {
    let store = ChronoMind::new(Config {
        dimensions: 2,
        temporal_weight: 0.0,
        ..Config::default()
    })
    .unwrap();

    let old_time = SystemTime::now() - Duration::from_secs(30 * 24 * 3600);
    store
        .insert(Memory::new(
            Vector::new("old_near", vec![1.0, 0.0]),
            MemoryAttributes {
                timestamp: old_time,
                ..MemoryAttributes::default()
            },
        ))
        .unwrap();
    store.insert(memory("fresh_far", vec![0.0, 1.0])).unwrap();

    let results = store.search(&[1.0, 0.0], 2).unwrap();
    assert_eq!(results[0].0.vector.id, "old_near");
}

#[test]
fn context_search_filters() {
    let store = ChronoMind::new(config(2)).unwrap();
    store
        .insert(memory_in_context("a", vec![1.0, 0.0], "alpha"))
        .unwrap();
    store
        .insert(memory_in_context("b", vec![1.0, 0.0], "beta"))
        .unwrap();

    let results = store.search_in_context("alpha", &[1.0, 0.0], 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.vector.id, "a");
}

#[test]
fn invalid_inputs_are_rejected() {
    let store = ChronoMind::new(config(3)).unwrap();

    assert!(matches!(
        store.insert(memory("short", vec![1.0])),
        Err(Error::InvalidDimensions {
            got: 1,
            expected: 3
        })
    ));
    assert!(matches!(
        store.insert(memory("nan", vec![1.0, f32::NAN, 0.0])),
        Err(Error::InvalidVector(_))
    ));
    assert!(matches!(
        store.search(&[1.0], 5),
        Err(Error::InvalidDimensions { .. })
    ));
    assert!(matches!(
        store.search(&[1.0, f32::INFINITY, 0.0], 5),
        Err(Error::InvalidVector(_))
    ));

    let mut bad_importance = memory("imp", vec![1.0, 0.0, 0.0]);
    bad_importance.attributes.importance = 2.0;
    assert!(matches!(
        store.insert(bad_importance),
        Err(Error::InvalidImportance(_))
    ));
}

#[test]
fn capacity_is_enforced_but_replacement_is_allowed() {
    let store = ChronoMind::new(Config {
        dimensions: 2,
        max_memories: 2,
        ..Config::default()
    })
    .unwrap();

    store.insert(memory("a", vec![1.0, 0.0])).unwrap();
    store.insert(memory("b", vec![0.0, 1.0])).unwrap();
    assert!(matches!(
        store.insert(memory("c", vec![1.0, 1.0])),
        Err(Error::CapacityExceeded(2))
    ));
    // Replacing an existing id does not hit the capacity check.
    store.insert(memory("a", vec![0.5, 0.5])).unwrap();
    assert_eq!(store.len(), 2);
}

#[test]
fn reinsert_merges_relationships() {
    let store = ChronoMind::new(config(2)).unwrap();

    let mut first = memory("a", vec![1.0, 0.0]);
    first.attributes.relationships = vec!["b".into()];
    store.insert(first).unwrap();

    let mut second = memory("a", vec![0.9, 0.1]);
    second.attributes.relationships = vec!["c".into(), "b".into()];
    store.insert(second).unwrap();

    let got = store.get("a").unwrap();
    assert_eq!(got.attributes.relationships, vec!["b", "c"]);
    assert_eq!(got.vector.data, vec![0.9, 0.1]);
}

#[test]
fn access_records_retrieval() {
    let store = ChronoMind::new(config(2)).unwrap();
    store.insert(memory("a", vec![1.0, 0.0])).unwrap();

    let before = store.get("a").unwrap();
    assert_eq!(before.attributes.access_count, 0);
    store.access("a").unwrap();
    let after = store.access("a").unwrap();
    assert_eq!(after.attributes.access_count, 2);
    assert!(store.access("missing").is_none());
}

#[test]
fn decay_reduces_importance_of_stale_memories() {
    let store = ChronoMind::new(config(2)).unwrap();
    let week_ago = SystemTime::now() - Duration::from_secs(7 * 24 * 3600);
    store
        .insert(Memory::new(
            Vector::new("stale", vec![1.0, 0.0]),
            MemoryAttributes {
                importance: 1.0,
                timestamp: week_ago,
                last_access: week_ago,
                ..MemoryAttributes::default()
            },
        ))
        .unwrap();

    store.apply_decay();
    let decayed = store.get("stale").unwrap().attributes.importance;
    assert!(
        decayed < 1.0e-3,
        "a week at rate 0.1/hour should decay ~to zero, got {decayed}"
    );
}

#[test]
fn decay_leaves_fresh_memories_nearly_intact() {
    let store = ChronoMind::new(config(2)).unwrap();
    let mut fresh = memory("fresh", vec![1.0, 0.0]);
    fresh.attributes.importance = 0.8;
    store.insert(fresh).unwrap();

    store.apply_decay();
    let importance = store.get("fresh").unwrap().attributes.importance;
    assert!((importance - 0.8).abs() < 0.01);
}

#[test]
fn consolidate_merges_near_duplicates() {
    let mut store = ChronoMind::new(Config {
        dimensions: 2,
        similarity_threshold: 0.99,
        ..Config::default()
    })
    .unwrap();

    let mut keep = memory("keep", vec![1.0, 0.0]);
    keep.attributes.importance = 0.9;
    keep.attributes.relationships = vec!["x".into()];
    let mut dup = memory("dup", vec![1.0, 0.001]);
    dup.attributes.importance = 0.2;
    dup.attributes.relationships = vec!["y".into()];
    let distinct = memory("distinct", vec![0.0, 1.0]);

    store.insert(keep).unwrap();
    store.insert(dup).unwrap();
    store.insert(distinct).unwrap();

    let absorbed = store.consolidate();
    assert_eq!(absorbed, 1);
    assert_eq!(store.len(), 2);
    assert!(store.get("dup").is_none());

    let survivor = store.get("keep").unwrap();
    assert_eq!(survivor.attributes.importance, 0.9);
    let mut links = survivor.attributes.relationships.clone();
    links.sort();
    assert_eq!(links, vec!["x", "y"]);
    assert!(store.get("distinct").is_some());
}

#[test]
fn related_walks_links_breadth_first_with_depth_cap() {
    let store = ChronoMind::new(config(2)).unwrap();
    let mut a = memory("a", vec![1.0, 0.0]);
    a.attributes.relationships = vec!["b".into()];
    let mut b = memory("b", vec![0.9, 0.1]);
    b.attributes.relationships = vec!["c".into()];
    let c = memory("c", vec![0.8, 0.2]);

    store.insert(a).unwrap();
    store.insert(b).unwrap();
    store.insert(c).unwrap();

    let one_hop: Vec<String> = store
        .related("a", 1)
        .into_iter()
        .map(|m| m.vector.id)
        .collect();
    assert_eq!(one_hop, vec!["b"]);

    let two_hops: Vec<String> = store
        .related("a", 2)
        .into_iter()
        .map(|m| m.vector.id)
        .collect();
    assert_eq!(two_hops, vec!["b", "c"]);
}

#[test]
fn context_summary_aggregates() {
    let store = ChronoMind::new(config(2)).unwrap();
    let mut a = memory_in_context("a", vec![1.0, 0.0], "ctx");
    a.attributes.importance = 0.4;
    let mut b = memory_in_context("b", vec![0.0, 1.0], "ctx");
    b.attributes.importance = 0.6;
    store.insert(a).unwrap();
    store.insert(b).unwrap();

    let summary = store.context_summary("ctx").unwrap();
    assert_eq!(summary.memory_count, 2);
    assert!((summary.average_importance - 0.5).abs() < 1e-6);
    assert_eq!(summary.centroid, vec![0.5, 0.5]);
    assert!(store.context_summary("empty").is_none());
}

#[test]
fn stats_reflect_contents() {
    let store = ChronoMind::new(config(2)).unwrap();
    let mut a = memory_in_context("a", vec![1.0, 0.0], "ctx1");
    a.attributes.relationships = vec!["b".into()];
    let b = memory_in_context("b", vec![0.0, 1.0], "ctx2");
    store.insert(a).unwrap();
    store.insert(b).unwrap();

    let stats = store.stats();
    assert_eq!(stats.total_memories, 2);
    assert_eq!(stats.total_components, 4);
    assert_eq!(stats.context_distribution["ctx1"], 1);
    assert_eq!(stats.context_distribution["ctx2"], 1);
    assert_eq!(stats.most_referenced, vec![("b".to_string(), 1)]);
}

#[test]
fn remove_deletes() {
    let store = ChronoMind::new(config(2)).unwrap();
    store.insert(memory("a", vec![1.0, 0.0])).unwrap();
    assert!(store.remove("a").is_some());
    assert!(store.remove("a").is_none());
    assert!(store.is_empty());
}
