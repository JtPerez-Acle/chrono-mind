use std::fs;
use std::io::Write;

use chronomind::{
    load_snapshot, save_snapshot, ChronoMind, Config, Error, Memory, MemoryAttributes, Vector,
};

fn sample_store() -> ChronoMind {
    let store = ChronoMind::new(Config {
        dimensions: 4,
        ..Config::default()
    })
    .unwrap();
    for i in 0..20 {
        let x = i as f32;
        store
            .insert(Memory::new(
                Vector::new(format!("m{i}"), vec![x, x + 1.0, x + 2.0, x + 3.0]),
                MemoryAttributes {
                    importance: (i as f32) / 20.0,
                    context: if i % 2 == 0 { "even" } else { "odd" }.into(),
                    ..MemoryAttributes::default()
                },
            ))
            .unwrap();
    }
    store
}

#[test]
fn snapshot_roundtrip_preserves_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.chrono");

    let store = sample_store();
    save_snapshot(&store, &path).unwrap();
    let loaded = load_snapshot(&path).unwrap();

    assert_eq!(loaded.len(), store.len());
    assert_eq!(loaded.config(), store.config());
    for original in store.snapshot() {
        let restored = loaded.get(&original.vector.id).unwrap();
        assert_eq!(restored, original);
    }
}

#[test]
fn loaded_store_is_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.chrono");
    save_snapshot(&sample_store(), &path).unwrap();

    let loaded = load_snapshot(&path).unwrap();
    let results = loaded.search(&[0.0, 1.0, 2.0, 3.0], 1).unwrap();
    assert_eq!(results[0].0.vector.id, "m0");
}

#[test]
fn bad_magic_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not_a_snapshot.bin");
    fs::write(&path, b"definitely not a chronomind snapshot").unwrap();

    assert!(matches!(
        load_snapshot(&path),
        Err(Error::InvalidSnapshot(_))
    ));
}

#[test]
fn truncated_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("short.bin");
    fs::write(&path, b"CHR").unwrap();

    assert!(matches!(
        load_snapshot(&path),
        Err(Error::InvalidSnapshot(_))
    ));
}

#[test]
fn unsupported_version_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.bin");
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(b"CHRONO1").unwrap();
    file.write_all(&[99]).unwrap(); // format version from the future
    drop(file);

    let err = load_snapshot(&path).unwrap_err();
    assert!(matches!(err, Error::InvalidSnapshot(_)));
    assert!(err.to_string().contains("99"));
}

#[test]
fn missing_file_is_io_error() {
    assert!(matches!(
        load_snapshot(std::path::Path::new("does/not/exist.chrono")),
        Err(Error::Io(_))
    ));
}
