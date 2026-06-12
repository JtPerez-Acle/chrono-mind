# ChronoMind

[![CI](https://github.com/JtPerez-Acle/chrono-mind/actions/workflows/ci.yml/badge.svg)](https://github.com/JtPerez-Acle/chrono-mind/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](Cargo.toml)

**A lock-free concurrent HNSW vector index — wait-free reads, lock-free
writes — with a temporal memory layer for AI agents on top.**

The contribution is the index. It is a Hierarchical Navigable Small World
graph whose searches are **wait-free** and whose writes are **lock-free**,
shareable across threads through a plain `&self` API — **no operation
anywhere in this crate blocks on a mutex or RwLock** — and the receipts
(loom, Miri, ThreadSanitizer, a differential oracle, all in CI) back the
claim. Measured against usearch and FAISS it holds recall and search
throughput; under concurrent reads-and-writes it keeps serving where a
locked index stalls (see [Against the field](#against-the-field)).

On top sits a **temporal memory layer** for agent use cases: vectors carry
importance, decay rate, and access history, and search blends geometric
distance with recency. This layer is an *application* of the index, and it
has one constraint worth stating up front. The node arena is append-only:
refreshing a memory's importance is a cheap atomic CAS, but **rewriting a
memory's embedding allocates a fresh slot** (the old one is tombstoned until
a snapshot reload compacts). Insert-and-refresh workloads are the sweet
spot; workloads that continuously *rewrite embeddings* grow memory between
compactions. Plan for periodic snapshot reloads, or use the index directly.

```rust
use chronomind::{ChronoMind, Config, Memory, MemoryAttributes, Vector};

let config = Config::builder().dimensions(4).build()?;
let store = ChronoMind::new(config)?;   // note: not `mut` — writes are &self

store.insert(Memory::new(
    Vector::new("first", vec![0.1, 0.2, 0.3, 0.4]),
    MemoryAttributes { importance: 0.8, context: "demo".into(), ..Default::default() },
))?;

let results = store.search(&[0.1, 0.2, 0.3, 0.4], 5)?;
# Ok::<(), chronomind::Error>(())
```

## The lock-free claim, precisely

Most "concurrent" vector stores wrap their index in a reader-writer lock.
ChronoMind does not. The claim, stated as exactly as the code delivers it:

> **Reads are wait-free** — an epoch-pinned search performs only atomic
> pointer loads: no CAS, no retry loop, no lock, regardless of concurrent
> writer activity. **Writes are lock-free** — publication is a single
> compare-and-swap; a writer retries only when another writer has
> *succeeded*, so the system always makes progress.

The design that makes it true:

```
            ┌────────────────────────────────────────────────┐
   search ─►│ entry point        one packed AtomicU64        │
            │   │                                            │
            │   ▼                                            │
            │ node arena         chunked, append-only —      │
            │   │                nodes never move, u32       │
            │   │                handles stay valid forever  │
            │   ▼                                            │
            │ neighbor lists     immutable COW slices,       │
            │                    swapped by CAS, reclaimed   │
            │                    by crossbeam-epoch          │
            └────────────────────────────────────────────────┘
```

- **Node arena** (`src/index/arena.rs`): slots are reserved with one
  `fetch_add` and published with a release-ordered `ready` flag. Nodes are
  never moved or freed while the index lives, so traversal needs no
  reclamation protocol for nodes at all.
- **COW neighbor lists** (`src/index/neighbors.rs`): each (node, layer)
  adjacency is an atomic pointer to an immutable slice. Writers build a
  modified copy and CAS it in; replaced slices are destroyed only after
  every thread that could have seen them has moved on (epoch-based
  reclamation).
- **Tombstone deletes**: removed nodes keep routing traffic but leave
  results. Compaction is a snapshot save/load, the standard HNSW tradeoff.
- **Temporal scoring stays out of the graph.** The index is purely
  geometric; recency reranks the candidate pool in the store layer. One
  formula, documented on `ChronoMind::search`, used everywhere:

  ```text
  score = (1 - w) · distance/2  +  w · (1 - e^(-rate · age_hours))
  ```

## How it's verified

Claims about concurrent code are cheap; ChronoMind ships its receipts:

| gate | what it proves |
|---|---|
| **loom** model checks | The CAS publish protocol and the arena's reserve/write/publish protocol admit no lost updates and no torn reads, verified across *all* reachable interleavings at small scale |
| **Miri** (strict provenance) | The `unsafe` blocks in the arena and COW lists are free of detected undefined behavior |
| **Recall gates** | recall@10 ≥ 0.95 vs brute force on embedding-like data (768-d, low intrinsic dimension) and 32-d uniform data — for the lock-free index *and* the locked baseline it's compared against |
| **Connectivity gate** | Uniform 768-d data at ef=200 still reaches ≥ 0.95: a failure here means broken graph construction, not hard data |
| **16-thread stress** | 104k mixed ops (75% search / 20% insert / 5% delete), then a full structural invariant sweep (no dangling handles, no duplicate links, no cap violations) and recall ≥ 0.90 over survivors |
| **Recall under churn** | Queries racing live concurrent inserts must keep recall ≥ 0.90 against pre-churn ground truth — search *quality* mid-mutation, not just memory safety |
| **Reclamation gates** | A counting global allocator proves epoch garbage is actually freed (1M churned slices plateau under 16 MB in flight) — and measures the known pathology: a guard pinned forever blocks reclamation until released |
| **Op-sequence fuzzing** | proptest drives arbitrary insert/remove/search/consolidate sequences with the structural invariant sweep as the oracle; a coverage-guided cargo-fuzz target runs the same harness in CI |
| **Differential oracle** | Arbitrary op sequences run against BOTH index implementations must match a brute-force linear-scan model *exactly* (ids, order, distances) in the exhaustive-ef regime — lost inserts, ghost tombstones, and distance corruption cannot hide |
| **Reproducible persistence** | Snapshot saves are atomic (temp file + fsync + rename) with a CRC32 body checksum: corruption is rejected, a crash mid-save can never destroy the previous snapshot — both gated by tests |
| **MSRV proof** | CI compiles the crate on Rust 1.82 so the badge is verified, not asserted |
| **ThreadSanitizer** | The stress suite under TSan on real scheduling — the complement to loom's models (suppressions cover only crossbeam-epoch's fence-based sync, never our code) |
| **ARM (weak memory)** | The full suite on aarch64 Linux in CI — x86's strong ordering can mask acquire/release mistakes; ARM hardware cannot |
| **CI** | All of the above on every push, Windows and Linux |

Run them yourself:

```bash
cargo test                                          # everything incl. recall + stress gates
RUSTFLAGS="--cfg loom" cargo test --test loom --release   # exhaustive interleavings
cargo +nightly miri test --lib index                # UB detection
```

## The baselines ship too

`src/index/rwlock_hnsw.rs` is the same HNSW algorithm behind a single
`RwLock` — deliberately kept, always compiled. It is the correctness
reference the lock-free index is tested against, and the A/B baseline for
the benchmarks. And because a single RwLock is the baseline everyone
beats, `src/index/sharded_rwlock.rs` adds the *fair* competitor: 16
independently locked shards with round-robin routing — the design a
practitioner would actually deploy. All three implement the same
`VectorIndex` trait; pick whichever you trust.

## Benchmarks

`cargo bench` runs three A/B workloads (lock-free vs RwLock baseline) at
1/2/4/8 threads over embedding-like 768-d data: `insert_throughput`,
`search_qps`, and `mixed_90_10` (90% search / 10% insert).

Measured on an i7-12700KF (12 cores), Windows 11, Rust stable, 768-d
embedding-like data, M=16 / efC=100 / efS=50. Three contenders: the
lock-free index, a single RwLock, and the fair competitor — 16
independently locked shards (`sharded16`). Ops/sec, criterion
mid-estimates from one coherent run:

**Concurrent inserts:**

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 3,859 | 4,123 | **10,130** |
| 2 | 7,839 | 4,034 | **20,204** |
| 4 | 15,286 | 4,028 | **39,264** |
| 8 | 28,300 | 4,038 | **71,152** |

Honest verdict: **sharding wins pure construction** — each insert searches
a graph 1/16th the size, and writers rarely collide across 16 locks. The
single RwLock is flat (one writer at a time); the lock-free index scales
near-linearly (7.3× from 1→8 threads) but pays full-graph construction cost
per insert.

**Pure search, zero writers:**

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 3,539 | 3,585 | 387 |
| 2 | 7,051 | 7,305 | 1,088 |
| 4 | 14,075 | 14,673 | 2,483 |
| 8 | **26,834** | **27,904** | 5,008 |

Sharding's bill comes due: every query pays 16 sub-searches plus a merge —
**9× slower at 1 thread, ~5× at 8** (its shards scale, but from a deep
hole). The uncontended RwLock keeps a small constant edge over lock-free
(~2–4%, epoch pinning is not free); that is the honest cost of wait-free
reads — and the same edge that makes RwLock fractionally faster at 1 thread
in the mixed table below.

**Mixed 90% search / 10% insert** (a search-dominated workload with steady
writes):

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 4,922 | 5,582 | 567 |
| 2 | 11,420 | 5,558 | 1,138 |
| 4 | 28,733 | 5,721 | 2,325 |
| 8 | **59,998** | 10,858 | 4,012 |

The workload that matters, and the only one with a single winner: the
lock-free index scales near-linearly to **60k ops/s**, beating the RwLock
**5.5×** and the sharded design **15×** at 8 threads. The RwLock is **flat
under writes** — its 10% inserts take the lock exclusively and throttle
every reader, so it never benefits from more threads. At 1 thread the
RwLock is marginally ahead (5,582 vs 4,922) for the same reason it leads
pure search: an uncontended read guard is a hair cheaper than epoch
pinning. Lock-free overtakes it the instant a second thread writes.

Numbers vary with hardware and run-to-run; the scaling *shapes* and
order-of-magnitude gaps are the durable signal. Full method and analysis:
[docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Against the field

Two honest comparisons against real ANN libraries — one where chronomind is
merely competitive, one where it wins by design.

**Single-thread search, standard datasets.** Through the PyO3 bindings
(`bindings/python/`), chronomind, usearch (SIMD C++), and FAISS (Meta's
`IndexHNSWFlat`) run over the ann-benchmarks datasets with their exact
ground truth — recall@10 vs single-thread QPS, the leaderboard convention:

| dataset | recall@10 (all three) | chrono search vs usearch | chrono search vs FAISS |
|---|---|---|---|
| GloVe-100 (1.18M, 100-d) | ~0.90 @ ef=400 | parity (chrono slightly ahead) | chrono +66% |
| NYTimes-256 (290k, 256-d) | ~0.90 @ ef=400 | parity (chrono ~10% behind) | FAISS ~2× ahead |

The durable read: **recall matches the field on both; search throughput
tracks usearch within ~10%, lead alternating; FAISS is dataset-dependent**
(slowest on GloVe, fastest on NYTimes). Single-thread *build* is 12–24×
slower than both — structural lock-free construction overhead, and honestly
the weakest number. chronomind already uses an AVX2+FMA distance kernel, so
search is not SIMD-limited. Full tables and caveats:
[bindings/python/results/](bindings/python/results/).

**Reads under concurrent writes — the scenario chronomind is built for**
(`cargo bench --bench concurrency --features bench-external`). 4 readers and
4 writers hammer one index; read throughput and tail latency are measured
idle, then under write load:

| system | read-QPS retention under writes | p99 idle → under writes |
|---|---:|---|
| **chronomind** (lock-free) | **65%** | 228 µs → 384 µs |
| usearch (C++) | 62% | 186 µs → 357 µs |
| one RwLock | **1%** | 237 µs → **79,310 µs** |

The single RwLock — the default way to make an index "concurrent" —
collapses: writers starve readers and p99 read latency explodes 334×.
chronomind holds 65% of its read throughput with p99 barely moving (the drop
is CPU sharing across 8 threads, not blocking), matching usearch. This is
the wait-free claim from the top of this README, measured against the field.

A third head-to-head (`cargo bench --bench external --features
bench-external`) runs instant-distance, hnsw_rs, and usearch over the same
synthetic data in-process; see [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## The temporal model

Every memory carries `MemoryAttributes`:

- **importance** `[0, 1]` — decays over time via `apply_decay()`
  (`exp(-rate · hours_since_last_access)`), refreshed by retrieval through
  `access()`. Decay is an atomic CAS per memory: the sweep runs concurrently
  with everything else.
- **decay_rate** — per-memory half-life control; `0.0` inherits the store's
  `base_decay_rate`.
- **context** — free-form grouping label; `search_in_context` scans the
  context exactly (sparse contexts never come back short),
  `context_summary` aggregates a centroid.
- **relationships** — directed links between memories; `related(id, depth)`
  walks them breadth-first. `consolidate()` merges near-duplicates
  (cosine similarity above `similarity_threshold`) and merges their links.

## CLI

```bash
cargo install --path .

chronomind save  --input vectors.json --output memories.chrono --dimensions 4 --normalize
chronomind query --file memories.chrono --vector "[0.1, 0.2, 0.3, 0.4]" --limit 3
chronomind query --file memories.chrono --vector "0.1,0.2,0.3,0.4" --context conversations
chronomind stats --file memories.chrono
```

Snapshots are a versioned, checksummed binary format (`CHRONO1` magic +
format byte + CRC32), written atomically — a crash mid-save leaves the
previous snapshot intact, and corruption is rejected at load rather than
half-loaded. The index is rebuilt on load.

## Design notes for the curious

- **Why is the library synchronous?** Because nothing in it waits on IO.
  The previous incarnation of this crate had `async fn` everywhere with
  zero awaited IO — async theater. A `Send + Sync` store with `&self`
  methods composes with any runtime (wrap calls in `spawn_blocking` if they
  ever measure as slow) and with plain threads.
- **Why does `consolidate` take `&mut self`?** It is an O(n²) maintenance
  pass. Exclusive access keeps its pairwise bookkeeping trivially correct;
  that is an API decision made visible in the signature, not a hidden lock.
  The contract for shared stores is **quiesce → `Arc::try_unwrap` (or
  `Arc::get_mut`) → consolidate → reshare**, and the doctest on
  `ChronoMind::consolidate` shows the exact legal calling pattern — the
  compiler rejecting `consolidate` through a live `Arc` is the contract
  being enforced, not an oversight.
- **Where are the `unsafe` blocks?** All in `src/index/arena.rs` and
  `src/index/neighbors.rs`, each with a `// SAFETY:` comment stating its
  invariant, all covered by loom + Miri. The rest of the crate is safe
  Rust.
- **Known limits**: tombstones accumulate until a snapshot reload — and
  since the arena is append-only, every *reinsert of an existing id* also
  burns a fresh arena slot, so update-heavy workloads grow memory until a
  snapshot reload compacts (at exhaustion, ~16.7M slots, inserts return a
  typed `Error::IndexFull` rather than panicking). Vector data is stored twice (once in the index
  arena for traversal, once in the store record for retrieval and context
  scans) — 2× vector memory traded for implementation simplicity; halving
  it is roadmap work. The capacity check is approximate under concurrent
  insertion (bounded overshoot); concurrent inserts cannot link to each
  other (standard for concurrent HNSW construction — covered by the
  stress recall gate).
  And epoch reclamation's classic weakness applies: **a guard pinned
  indefinitely blocks garbage collection**, so deferred slices accumulate
  without bound until it unpins. This is measured behavior, not theory —
  `tests/reclamation_test.rs` pins a hostage guard, watches garbage grow
  past 4 MB, releases it, and watches the pile drain. Guards in this
  crate live for the duration of one operation, so the pathology requires
  misusing the hidden primitive API directly.

Full architecture and milestone history: [docs/DESIGN.md](docs/DESIGN.md).

## Roadmap

- Write-ahead log persistence → crash safety and time-travel queries
- MCP server interface → drop-in persistent memory for AI agents
- Graph serialization → O(1)-ish snapshot loads
- Memory tiers (working / episodic / semantic) with consolidation policies

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option. Contributions are welcome under the same terms.
