# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
  Miri (strict provenance) over the unsafe primitives, seeded recall gates
  (≥ 0.95 vs brute force) for both indexes, a 768-d connectivity gate, a
  16-thread/104k-op stress gate with full graph-invariant sweep, and
  proptest property tests.
- Criterion A/B benchmarks (`cargo bench`): insert / search / mixed
  workloads across 1–8 threads, lock-free vs baseline.
- GitHub Actions CI: fmt, clippy `-D warnings`, tests, doc build, loom and
  Miri jobs, on Windows and Linux.
- HNSW correctness fixes over the old code: proper layer assignment
  (`floor(-ln(u)·mL)`, was capped at vector dimensionality), greedy
  descent through all layers (was missing), correct heap orientation (the
  old code evicted its *best* candidates), `f32::total_cmp` ordering (the
  old code could NaN-poison comparisons), Algorithm 4 diversity-aware
  neighbor selection.

### Removed
- `hnsw_rs` dependency (broke Windows builds; wrapped but effectively
  unused), OpenTelemetry stack, tokio, async-trait, memmap2, parking_lot,
  and six other unused dependencies.
- Dead files that were never part of the module tree, two zero-implementor
  `VectorStorage` traits, the unused in-house `TemporalHNSW`, and the
  fabricated benchmark results that described them.

## [0.1.0] - 2025-01-04

Initial public iteration (as `vector-store`): temporal vector storage with
HNSW-based search, memory decay, context grouping, relationship tracking,
JSON persistence, and a CLI. Retrospectively: the concurrency and
benchmark claims of this version did not hold; see 0.2.0.
