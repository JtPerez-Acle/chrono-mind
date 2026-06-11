//! End-to-end tests of the `chronomind` binary, driven through the real CLI.

use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_chronomind"))
        .args(args)
        .output()
        .expect("binary should execute")
}

fn save_sample(snapshot: &Path) -> Output {
    run(&[
        "save",
        "--input",
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/sample_vectors.json"),
        "--output",
        snapshot.to_str().unwrap(),
        "--dimensions",
        "4",
        "--normalize",
    ])
}

#[test]
fn save_query_stats_happy_path() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot = dir.path().join("cli.chrono");

    let save = save_sample(&snapshot);
    assert!(
        save.status.success(),
        "save failed: {}",
        String::from_utf8_lossy(&save.stderr)
    );
    assert!(snapshot.exists());

    let query = run(&[
        "query",
        "--file",
        snapshot.to_str().unwrap(),
        "--vector",
        "[0.1, 0.2, 0.3, 0.4]",
        "--limit",
        "3",
        "--normalize",
    ]);
    assert!(
        query.status.success(),
        "query failed: {}",
        String::from_utf8_lossy(&query.stderr)
    );
    let stdout = String::from_utf8_lossy(&query.stdout);
    assert!(stdout.contains("vector1"), "unexpected output: {stdout}");

    let stats = run(&["stats", "--file", snapshot.to_str().unwrap()]);
    assert!(stats.status.success());
    let stdout = String::from_utf8_lossy(&stats.stdout);
    assert!(
        stdout.contains("Total memories"),
        "unexpected output: {stdout}"
    );
}

#[test]
fn query_supports_comma_separated_vectors_and_context_filter() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot = dir.path().join("cli.chrono");
    assert!(save_sample(&snapshot).status.success());

    let query = run(&[
        "query",
        "--file",
        snapshot.to_str().unwrap(),
        "--vector",
        "0.1, 0.2, 0.3, 0.4",
        "--context",
        "context_a",
    ]);
    assert!(query.status.success());
    let stdout = String::from_utf8_lossy(&query.stdout);
    assert!(stdout.contains("context_a"));
    assert!(!stdout.contains("context_b"));
}

#[test]
fn missing_input_fails_with_helpful_error() {
    let output = run(&["save", "--input", "nope.json", "--output", "out.chrono"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error:"), "unexpected stderr: {stderr}");
}

#[test]
fn querying_a_non_snapshot_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let bogus = dir.path().join("bogus.chrono");
    std::fs::write(&bogus, "not a snapshot").unwrap();

    let output = run(&[
        "query",
        "--file",
        bogus.to_str().unwrap(),
        "--vector",
        "[0.1]",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("snapshot"), "unexpected stderr: {stderr}");
}
