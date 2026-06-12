# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.5] - 2026-06-12

First published release. 0.2.0 was the internal ground-up rebuild (below);
0.2.5 is that work hardened, benchmarked against the field, and shipped.

### Added
- **Python bindings** (`bindings/python/`): a maturin/PyO3 crate exposing
  the lock-free index to Python for benchmarking. Deliberately not a
  workspace member — its own Cargo.lock, so the pyo3/numpy tree never
  reaches the published library or its CI.
- **Real-dataset benchmark driver** (`bindings/python/ann_bench.py`): loads
  ann-benchmarks HDF5 datasets and traces recall@k vs single-thread QPS
  against bundled ground truth, for chronomind and whichever of usearch /
  FAISS / hnswlib are importable.
- **Reads-under-writes concurrency benchmark** (`benches/concurrency.rs`,
  behind `bench-external`): read throughput and p99 latency, idle vs under
  concurrent writers, for chronomind vs sharded16 vs one RwLock vs usearch —
  the scenario the lock-free design exists for.
- Measured competitive position folded into the README and
  `docs/BENCHMARKS.md`, with per-dataset results under
  `bindings/python/results/`: recall matches usearch and FAISS on GloVe-100
  and NYTimes-256; single-thread search tracks usearch within ~10%; build
  is 12–24× slower (structural); under concurrent writes chronomind retains
  ~65% read throughput (matching usearch) where a single RwLock collapses to
  1% with p99 up 334×.

### Changed
- **Cosine distance normalizes once.** `DistanceMetric` gains
  `preprocess` + `distance_prepared` (defaults: identity + `distance`);
  `CosineDistance` unit-normalizes at insert/query so the hot path is a bare
  dot product instead of recomputing both operands' norms per call. Both
  indexes store the prepared vector; the differential oracle mirrors the
  same arithmetic so its exact-rank check still holds.
- **MSRV 1.75 → 1.82.** The committed `Cargo.lock` is format v4, which Cargo
  cannot parse before 1.78; 1.82 is the floor CI now verifies. The library
  code itself still compiles far lower (papaya and seize need only 1.72).

### Fixed
- First-run CI: the ThreadSanitizer suppression file targeted
  crossbeam-epoch (which is TSan-clean here); the real false positives come
  from seize's `sys_membarrier` reclamation (reached via papaya), which TSan
  cannot model. Retargeted the suppressions to seize's collector/membarrier
  with the full rationale; chronomind's own code remains unsuppressed.
- Public docs on `ShardedRwLockHnsw` linked the private `SHARDS` const via an
  intra-doc link, failing `cargo doc -D warnings`. Demoted to a code span.
- `mixed_90_10` benchmark methodology: corpus, queries, and inserts now
  share one embedding subspace. Previously queries lived in a subspace
  foreign to the corpus, so the workload's accumulated same-subspace inserts
  seeded an artificially easy cluster — inflating throughput and producing
  non-monotonic thread scaling. The corrected numbers are lower and stable
  (README and docs/BENCHMARKS.md updated).
- README now leads with the lock-free index as the contribution and the
  temporal store as the application on top, and states the append-only
  arena's reinsert cost (embedding rewrites allocate a fresh slot) up front.

## [0.2.0] - 2026-06-11

Ground-up rework. The previous codebase claimed to be lock-free and
concurrent while serializing every write behind `RwLock<HashMap>`s and
`&mut self` APIs; it shipped two parallel implementations of nearly every
type, dead modules that never compiled, and benchmarks measuring an index
the public API never used. 0.2.0 deletes all of it and makes the claims
true instead. See `docs/DESIGN.md` for the full audit and design.

### Changed (breaking — everything)
- Crate renamed `vector-store` → `chronomind`; binary renamed to
  `chronomind`.
- License changed from Apache-2.0 only to dual **MIT OR Apache-2.0**.
- The library is now fully synchronous: tokio and async-trait removed.
  Every former `async fn` was fake async (no awaited IO existed).
- Single set of types: one `Vector`, one `Memory` (was `TemporalVector`
  with duplicated fields), one `Config`, one store, one error enum.
- Snapshot persistence replaces ad-hoc JSON: versioned binary format
  (`CHRONO1` magic + format byte, bincode body). Old `.store` files are
  not readable.
- Temporal scoring extracted into one documented formula applied as a
  rerank over index candidates; the index itself is purely geometric.

### Added
- **Lock-free concurrent HNSW index** (`index::LockFreeHnsw`): wait-free
  reads, lock-free writes. Chunked append-only node arena, copy-on-write
  neighbor lists with crossbeam-epoch reclamation, packed atomic entry
  point, tombstone deletes.
- Fully concurrent `&self` store API: insert, search, get, access, remove,
  decay sweeps and stats all run from any number of threads; importance
  and access tracking are atomics (`papaya` maps at the id boundary).
- `RwLockHnsw`: the same algorithm behind a single lock, kept as the
  correctness reference and benchmark baseline.
- Verification suite: loom model checks of the CAS and arena protocols,
  Miri over the unsafe primitives (strict provenance on the arena, Tree
  Borrows on epoch-touching code), seeded recall gates (≥ 0.95 vs brute
  force) for both indexes, a 768-d connectivity gate, a 16-thread/104k-op
  stress gate with full graph-invariant sweep, a recall-under-churn gate
  (queries racing live writers must hold ≥ 0.90), reclamation gates under
  a counting global allocator (epoch garbage provably plateaus and drains;
  the pinned-guard pathology is measured, not theorized), op-sequence
  fuzzing with the invariant sweep as oracle (proptest + a cargo-fuzz
  target), and a differential oracle pinning both index implementations
  to exact agreement with a linear-scan model in the exhaustive-ef regime.
- `ShardedRwLockHnsw`: a 16-shard locked baseline — the fair competitor —
  included in the benchmark matrix.
- Crash-safe snapshot persistence (format v2): atomic temp-file +
  fsync + rename writes and a CRC32 body checksum verified before
  deserialization.
- Index storage exhaustion surfaces as `Error::IndexFull` instead of a
  panic (`VectorIndex::insert` returns `Option`).
- Criterion A/B benchmarks (`cargo bench`): insert / search / mixed
  workloads across 1–8 threads, lock-free vs RwLock vs sharded16.
- External head-to-head (`cargo bench --bench external --features
  bench-external`) vs instant-distance, hnsw_rs, and usearch: search
  throughput at parity with usearch within run noise at recall 0.998,
  3–4× faster than the pure-Rust crates; full table and caveats in
  docs/BENCHMARKS.md.
- GitHub Actions CI: fmt, clippy `-D warnings`, tests, doc build
  (`-D warnings`, `deny(missing_docs)`), loom, two Miri jobs, MSRV 1.75
  check, aarch64 (weak memory model) test job, ThreadSanitizer on the
  stress suite, and a fuzz smoke run — Windows and Linux.
- HNSW correctness fixes over the old code: proper layer assignment
  (`floor(-ln(u)·mL)`, was capped at vector dimensionality), greedy
  descent through all layers (was missing), correct heap orientation (the
  old code evicted its *best* candidates), `f32::total_cmp` ordering (the
  old code could NaN-poison comparisons), Algorithm 4 diversity-aware
  neighbor selection.

### Fixed
- `apply_decay` no longer compounds under periodic invocation: sweeps now
  apply disjoint time intervals (per-record high-water mark with a CAS
  gate), so the decay curve depends on elapsed time, not call frequency.
  The compounding behavior was inherited from the pre-0.2 code.
- A search racing a reinsert of the same id can no longer return both
  versions of one memory (results are deduplicated by external id).

### Removed
- `hnsw_rs` dependency from the build (broke Windows builds at 0.1.19;
  wrapped but effectively unused — it returns, fixed upstream at 0.3.4,
  as an optional bench-external dev contender), OpenTelemetry stack,
  tokio, async-trait, memmap2, parking_lot, and six other unused
  dependencies.
- Dead files that were never part of the module tree, two zero-implementor
  `VectorStorage` traits, the unused in-house `TemporalHNSW`, and the
  fabricated benchmark results that described them.

## [0.1.0] - 2025-01-04

Initial public iteration (as `vector-store`): temporal vector storage with
HNSW-based search, memory decay, context grouping, relationship tracking,
JSON persistence, and a CLI. Retrospectively: the concurrency and
benchmark claims of this version did not hold; see 0.2.0.
