# ChronoMind v0.2 Design Document

**Status:** Approved for implementation
**Goal:** Transform ChronoMind into a portfolio-grade showcase of production Rust: a temporal
vector store whose headline claim — *lock-free concurrent HNSW* — is actually true, verifiable,
and benchmarked against its own locked baseline.

This document is the single source of truth for the rework. Every decision needed for
autonomous execution is resolved here. Sections marked **GATE** are acceptance criteria that
must pass before moving to the next phase.

---

## 1. Goals and non-goals

### Goals
1. **Honest, defensible claims.** Every statement in the README must be backed by code,
   tests, or reproducible benchmarks. No exceptions.
2. **A genuinely lock-free temporal HNSW index**: wait-free reads (epoch-pinned traversal,
   no CAS in the read path), lock-free writes (CAS with bounded retry, copy-on-write
   neighbor lists).
3. **Verification as a feature**: `loom` interleaving tests for the concurrent primitives,
   recall tests against brute force, multi-threaded stress tests, criterion A/B benchmarks
   vs. the locked baseline, CI on Windows + Linux.
4. **A clean, single-implementation codebase** — one vector type, one config, one store,
   one index.

### Non-goals (explicitly out of scope; listed as future work in README)
- WAL / time-travel queries
- MCP server interface
- Distributed deployment / sharding
- GPU acceleration
- Publishing to crates.io (prepare metadata, but publishing is the owner's manual action)

---

## 2. Project-level decisions

| Decision | Resolution | Rationale |
|---|---|---|
| Crate/lib/bin name | `chronomind` | Matches repo identity; `vector-store` is generic and squatted territory. Check crates.io availability at publish time; the repo does not depend on it. |
| Version | `0.2.0` | Cargo.toml is the source of truth (currently 0.1.0). Commit-message versions (v0.4/v0.5) were never released; do not honor them. |
| License | Dual `MIT OR Apache-2.0` | Owner wants MIT; dual licensing is the Rust-ecosystem convention and includes MIT. Add `LICENSE-MIT` + `LICENSE-APACHE`, delete single `LICENSE`, update `Cargo.toml` `license` field and README badge. |
| Async | **Remove entirely.** Core library is synchronous. | Every `.await` in the current code is fake — there is zero awaited IO. The store is in-memory compute. A lock-free `Send + Sync` store is *better* without async: callers on any runtime can use it directly. Removing tokio also cuts compile time dramatically. Document this reasoning in the README (it is itself a portfolio point). |
| OpenTelemetry | **Remove** (`opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`). Keep `tracing` for structured logging. | OTel is heavyweight ceremony with no consumer here. `tracing` spans give the same observability story at a fraction of the cost. |
| `hnsw_rs` dependency | **Remove.** | Breaks the Windows build (u32/u64 c_ulong mismatch); only used by the vestigial wrapper in `memory/temporal.rs`; superseded by our own index. |
| Edition / MSRV | Edition 2021, MSRV 1.75, stated in Cargo.toml (`rust-version`) and README. | Matches existing badge claim. |
| Commit policy | One commit per phase milestone (listed in §8), conventional-commits style. | Reviewable history is part of the portfolio. |

### Dependency budget after cleanup

Runtime: `thiserror`, `tracing`, `serde`, `serde_json`, `bincode`, `clap`, `indicatif`,
`crossbeam-epoch`, `papaya` (lock-free hashmap for the ID boundary), `rand` (layer assignment).
Dev: `criterion`, `proptest`, `loom`, `tempfile`, `rayon` (stress-test thread pools).
Everything else currently in Cargo.toml is dropped (`tokio`, `async-trait`, `memmap2`,
`lazy_static`, `once_cell`, `parking_lot`, `futures`, OTel crates, etc.). If a dropped crate
turns out to be needed, justify it in the commit message.

> Note on `papaya` vs `dashmap`: dashmap uses **sharded RwLocks** — using it would undermine
> the lock-free claim at the API boundary. `papaya` provides genuinely lock-free reads with
> epoch-based reclamation, consistent with the index design. If `papaya` proves unsuitable
> in practice, fall back to dashmap **and say so honestly in the README** ("string→id mapping
> uses sharded locking; the index itself is lock-free").

---

## 3. Target architecture

```
src/
  lib.rs            public API surface + crate docs (doc examples MUST compile via doctest)
  config.rs         Config (single struct, builder pattern, validate())
  error.rs          Error enum (thiserror) + Result alias
  types.rs          Vector, Memory (renamed from TemporalVector), MemoryAttributes — deduped
  metric.rs         DistanceMetric trait + CosineDistance (SIMD AVX2 + scalar fallback)
  store.rs          ChronoMind — the public store: temporal scoring, decay, contexts,
                    relationships, consolidation. Owns the index + memory map.
  index/
    mod.rs          index API types (SearchResult, IndexStats)
    arena.rs        chunked append-only node arena (lock-free)
    neighbors.rs    COW neighbor lists over crossbeam-epoch (the core primitive)
    hnsw.rs         lock-free HNSW (insert/search/delete-by-tombstone)
    rwlock_hnsw.rs  locked baseline implementation, cfg(feature = "baseline")
  persistence.rs    bincode snapshot (magic header CHRONO1, format version byte)
  main.rs           CLI (save/query/stats), binary name `chronomind`
```

### Deleted outright (Phase 1)
- `src/storage/temporal.rs` — dead file, not in module tree, cannot compile
- `src/storage/config.rs` — dead file, not in module tree
- `src/memory/traits.rs` + the `VectorStorage` trait in `storage/mod.rs` — zero implementors
- The `Hnsw` wrapper struct in `src/memory/temporal.rs` (hnsw_rs)
- `src/utils/monitoring.rs` (OTel-coupled; superseded by tracing + IndexStats)
- `benches/archive/` — archived dead code does not belong in the repo; git history preserves it
- `vectors.store` at repo root — generated artifact, add to .gitignore
- `docs/CODE_REVIEW.md`, `docs/PERFORMANCE_ANALYSIS.md`, `docs/TESTS.md`,
  `docs/BENCHMARKS_STRUCTURE.md`, `docs/DATA_FLOW.md` — describe the old world; delete rather
  than archive (git history is the archive). `TODO.md` replaced by a short ROADMAP section
  in README.

### Type consolidation (Phase 1)
- **One** `Vector { id: String, data: Vec<f32> }` in `types.rs`. The `metadata: Option<serde_json::Value>` field from `storage/mod.rs` is dropped (nothing uses it).
- `TemporalVector` → renamed `Memory`. Remove the duplicated top-level
  `created_at`/`last_accessed`/`access_count` fields — `attributes` is the single source of
  truth. Add `Default for MemoryAttributes` (importance 0.5, decay_rate from config default,
  empty context/relationships, now() timestamps) so README examples compile.
- **One** config: `Config` in `config.rs` (the current `core::config::MemoryConfig` survives,
  renamed; the dead `storage/config.rs` variant dies). Add `ef_search`, `ef_construction`,
  `max_connections` (M) — index params live in the same config with an `IndexParams` sub-struct.
- **One** `ContextSummary`, **one** `MemoryStats` (the `types.rs` versions win; reconcile
  fields to what `store.rs` actually computes).

---

## 4. Phase 2 — Correct single-threaded HNSW (the verification baseline)

Implement standard HNSW per Malkov & Yashunin (2018), single-threaded first, inside
`index/rwlock_hnsw.rs` structure (plain `RwLock` wrapping; this *becomes* the baseline for
Phase 3 A/B benchmarks — keep it under `feature = "baseline"` afterward).

### Algorithm spec (fixes the five bugs found in review)
1. **Layer assignment:** `l = floor(-ln(uniform(0,1)) * mL)` with `mL = 1/ln(M)`.
   Never coupled to vector dimensionality (current code caps layers at `max_dimensions` — bug).
2. **Insert:** greedy descent from the entry point through layers `top..l+1` with ef=1;
   then for layers `min(l, top)..0`, search with `ef_construction`, select `M` neighbors
   (simple nearest selection first; the paper's heuristic — Algorithm 4 — is a stretch goal),
   link bidirectionally, prune any neighbor exceeding `M_max` (`M` on upper layers, `2M` on
   layer 0) back to its nearest set.
3. **Search:** greedy descent from entry through all layers to layer 0 (current code jumps
   straight to layer 0 — bug), then ef_search-bounded best-first search at layer 0.
4. **Heaps:** explicit `min-heap by distance` for the candidate frontier and `max-heap by
   distance` for the result set (current code uses one inverted ordering for both and evicts
   the *best* results — bug). Use `std::cmp::Reverse` wrappers; wrap distances in a
   `TotalF32(f32)` newtype implementing `Ord` via `f32::total_cmp` (kills the NaN/divide-by-
   temporal-score poisoning — bug).
5. **Temporal scoring is NOT in the graph.** The index is purely geometric. Temporal
   weighting happens in `store.rs` as a rerank: fetch `ef = max(ef_search, 3·k)` candidates,
   score `combined = (1−w)·distance_norm + w·(1−exp(−decay_rate·age_secs))`, sort, truncate
   to `k`. One formula, defined in exactly one function, documented in rustdoc. Rationale:
   temporal scores change continuously; baking them into traversal corrupts graph invariants
   and makes results time-dependent in untestable ways (this was the source of the heap
   ordering confusion).

### GATE — Phase 2 exit criteria
- `recall@10 ≥ 0.95` vs brute-force on seeded-random unit vectors: N=2,000, dims ∈ {32, 768},
  20 queries, fixed RNG seed (test must be deterministic).
- Property test (proptest): every inserted vector is findable as its own nearest neighbor
  with distance < 1e-5.
- Zero-vector, NaN-component, and wrong-dimension inserts are rejected with typed errors.
- `cargo test` green on Windows; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean.

---

## 5. Phase 3 — Lock-free index (the headline)

### Data layout
- **Arena** (`index/arena.rs`): chunked append-only storage. Chunks of 4,096 nodes,
  chain of `AtomicPtr<Chunk>`; slot allocation via `AtomicU32` bump counter (fetch_add).
  Node IDs are `u32` arena indices. Nodes contain: the vector data (inline `Box<[f32]>`),
  the node's top layer, a tombstone `AtomicBool`, and per-layer neighbor-list heads.
  Nodes are never moved or freed while the index lives → readers need no GC for nodes
  themselves, only for neighbor lists.
- **Neighbor lists** (`index/neighbors.rs`) — *the* concurrency primitive:
  `crossbeam_epoch::Atomic<NeighborSlice>` per (node, layer), where `NeighborSlice` is an
  immutable `{ len: u32, ids: [u32; M_max] }`. Readers: `epoch::pin()`, load, iterate — no
  CAS, no locks (wait-free). Writers: load current, build modified copy, `compare_exchange`;
  on failure reload and retry; old slice retired via `defer_destroy`.
- **Entry point:** single `AtomicU64` packing `(node_id: u32, top_layer: u32)`; CAS-updated
  when an insert lands a higher layer.
- **ID boundary:** `papaya::HashMap<String, u32>` (external string IDs → arena indices) and
  the memory attribute map `papaya::HashMap<u32, MemoryRecord>` live in `store.rs`, not in
  the index. `importance` and `access_count` inside `MemoryRecord` are `AtomicU32`
  (f32 via `to_bits`/`from_bits`) so decay sweeps and access bumps are lock-free CAS loops —
  no write lock for decay.

### Operations
- **Insert:** (1) bump-allocate arena slot, write node data (private until published);
  (2) read-only graph search for neighbor candidates per layer (epoch-pinned);
  (3) set own neighbor lists (uncontended — node not yet visible);
  (4) publish: insert into papaya id-map, then CAS backlinks into each neighbor's list,
  pruning to `M_max` in the copied slice when full; (5) CAS entry point if higher.
  Failure mode: CAS retry loops are bounded by graph degree; no retry loop allocates
  unboundedly (reuse a scratch buffer).
- **Search:** entirely read-only; epoch pin for the duration of layer-0 search only
  (re-pin per layer to keep epochs short). Tombstoned nodes are traversed (their edges
  still route) but excluded from results.
- **Delete:** set tombstone. Physical compaction = snapshot save + reload (documented
  tradeoff, standard practice — hnswlib does the same).

### Claim wording (use exactly this in README; it is what the code delivers)
> Reads are **wait-free** (epoch-pinned pointer loads, no CAS, no retries). Writes are
> **lock-free** (CAS with retry; system-wide progress guaranteed, individual writers may
> retry under contention). No operation ever blocks on a mutex or RwLock.

### GATE — Phase 3 exit criteria
- **Loom tests** (`cfg(loom)`, separate test profile) for the COW neighbor primitive:
  (a) two concurrent backlink writers — final list contains both or one with the other
  pruned-by-policy, never lost-update or torn read; (b) writer + reader — reader sees
  either old or new slice, never a freed one. If crossbeam-epoch's loom integration is
  impractical, model the primitive over `loom::sync::atomic::AtomicPtr` with a test-only
  reclamation stub — the linearizability of the CAS protocol is what's being verified,
  not crossbeam itself. Document whichever route was taken.
- **Stress test:** 16 threads, ≥100k mixed ops (80% search / 20% insert), then invariant
  sweep: all neighbor ids < arena len, no duplicate ids in any list, recall@10 ≥ 0.90
  against brute force over surviving (non-tombstoned) nodes.
- **Miri** run on the non-loom unit tests of `arena.rs` and `neighbors.rs`
  (`cargo +nightly miri test -p chronomind arena neighbors`) — zero UB reports. If nightly
  is unavailable in CI, run locally and record the result in the PR/commit description.
- Recall gate from Phase 2 passes unchanged against the lock-free index.
- `unsafe` blocks: each carries a `// SAFETY:` comment stating the invariant; total count
  kept minimal and listed in the README's design notes section.

---

## 6. Benchmarks (Phase 4)

Criterion benches replacing the entire current `benches/` tree (delete it; it benches the
dead index with fabricated baselines):

1. `insert_throughput`: ops/sec, threads ∈ {1, 2, 4, 8}, N=50k, dim=768 — lock-free vs
   `baseline` feature RwLock implementation.
2. `search_qps`: same matrix, ef_search=50, k=10.
3. `mixed_90_10`: 90% search / 10% insert, the realistic agent-memory workload.
4. `recall_curve`: recall@10 vs ef_search ∈ {10, 25, 50, 100} (quality, single-threaded).

README gets a results table generated from a real run with hardware stated
("Ryzen X / Windows 11, your mileage varies") and exact reproduction commands. Delete all
existing README/BENCHMARKS.md numbers — they describe code that no longer exists and were
measured on the dead index. `docs/BENCHMARKS.md` is rewritten from actual criterion output.

---

## 7. CLI, persistence, docs, CI (Phase 5)

- **Persistence:** bincode snapshot of memories (id, vector, attributes) with magic
  `CHRONO1` + format version byte. Index is rebuilt on load (document the cost; graph
  serialization is future work). CLI `save` accepts the existing JSON input format
  (`examples/sample_vectors.json` keeps working); the old JSON `.store` format is dropped —
  v0.2 is a breaking release.
- **CLI:** binary renamed `chronomind`; keep `save`/`query`/`stats` semantics, port off
  tokio (plain `fn main`).
- **README rewrite:** honest overview; the lock-free design section (arena + COW + epochs,
  with a small diagram); verification story (loom/miri/recall/stress); benchmark table;
  "when to use / when not to" section retained (it's good); roadmap (WAL, MCP server,
  graph persistence) replacing TODO.md; MSRV, license badges fixed (MIT OR Apache-2.0).
- **USER_GUIDE.md / DEVELOPER_QUICKSTART.md / API.md / HNSW.md:** rewrite to match reality —
  or fold into README + rustdoc if they'd be thin. Rustdoc on all public items;
  `#![deny(missing_docs)]` on the crate.
- **CI (GitHub Actions):** `.github/workflows/ci.yml` — matrix {windows-latest,
  ubuntu-latest} × stable: fmt-check, clippy `-D warnings`, test (default features),
  test `--features baseline`, doc build with `-D warnings`. Separate job: loom tests
  (`RUSTFLAGS="--cfg loom"`, ubuntu only). CI badge in README.
- **CHANGELOG.md:** collapse the sprawling Unreleased section into an honest `0.2.0` entry:
  "ground-up rework; removed false lock-free/async claims and made the lock-free claim true."

---

## 8. Execution order and commit milestones

| # | Milestone (one commit each) | Contents | Gate |
|---|---|---|---|
| 1 | `chore!: demolition` | Delete dead files/deps/benches/docs per §3; collapse types; drop tokio/OTel/hnsw_rs; crate renamed `chronomind` 0.2.0; dual license; store temporarily backed by brute-force linear scan so the crate builds green at every commit | builds + tests compile and pass on Windows |
| 2 | `feat: correct HNSW baseline` | §4 implementation + recall/property tests | Phase 2 GATE |
| 3 | `feat: lock-free arena + COW neighbor lists` | §5 primitives + loom + miri | loom/miri green |
| 4 | `feat: lock-free HNSW index` | §5 full index, store wired to it, baseline behind feature flag | Phase 3 GATE |
| 5 | `feat: benches + stress` | §6 | numbers reproduced twice, variance sane |
| 6 | `docs: README/CHANGELOG/guides rewrite + CI` | §7 | CI green on both OSes |

Rules for autonomous execution:
- Every milestone leaves `main` (branch `rust`) buildable and tested on Windows.
- If a gate fails, fix forward within the phase; do not proceed with a failing gate.
- If a design assumption proves wrong mid-implementation (e.g., papaya API gap), choose the
  closest alternative that preserves the *claims*, document the deviation in this file under
  a "Deviations" appendix, and continue.
- No force-pushes; no history rewrites.

---

## Appendix A — Verified findings this design responds to (June 2026 audit)

1. `MemoryStorage` (the real API) uses the `hnsw_rs` crate; the in-house `TemporalHNSW` is
   only reachable from tests/benches. Benchmarks measured the unused index.
2. `src/storage/temporal.rs` + `src/storage/config.rs` are not in the module tree (dead);
   the former contains a tokio RwLock read→write deadlock and code that cannot compile.
3. `Vector` defined 3×, `VectorStorage` trait 2× (zero impls), `ContextSummary` 3×,
   `MemoryStats` 2×, config 2×.
4. Concurrency: `&mut self` on write APIs defeats interior locks; HNSW insert holds two
   global write locks for full traversal; nothing is lock-free.
5. In-house HNSW bugs: inverted heap ordering evicts best results; no layer descent in
   search; layer count capped by `max_dimensions` (default 3); `Candidate` ordering divides
   by decaying temporal score → NaN risk; insert searches with ef=1.
6. `hnsw_rs` wrapper: hardcoded 10k max elements, O(n) reverse-id lookup per result,
   no deletion (ghost results filtered silently).
7. README example calls nonexistent `MemoryAttributes::default()`; license says Apache-2.0
   while the owner intends MIT; Cargo version 0.1.0 vs commit-claimed v0.5; crate named
   `vector-store`; `hnsw_rs` 0.1.19 breaks the Windows build (c_ulong width).
