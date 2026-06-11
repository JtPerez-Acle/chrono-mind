# ChronoMind

[![CI](https://github.com/JtPerez-Acle/chrono-mind/actions/workflows/ci.yml/badge.svg)](https://github.com/JtPerez-Acle/chrono-mind/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](Cargo.toml)

**A temporal vector store for AI agent memory, built on a genuinely lock-free
concurrent HNSW index.**

ChronoMind stores embedding vectors together with temporal attributes —
creation time, importance, decay rate, access history — and ranks search
results by a documented blend of geometric distance and recency. Memories
decay, near-duplicates consolidate, and related memories link into graphs.
The entire store is shareable across threads through a plain `&self` API:
**no operation anywhere in this crate blocks on a mutex or RwLock.**

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
| 1 | 2,471 | 2,498 | **7,984** |
| 2 | 4,538 | 2,751 | **14,220** |
| 4 | 9,488 | 2,775 | **23,262** |
| 8 | 18,639 | 2,853 | **44,913** |

Honest verdict: **sharding wins pure construction** — each insert searches
a graph 1/16th the size, and writers rarely collide across 16 locks. The
single RwLock is flat; the lock-free index scales near-linearly (7.5×
self-scaling) but pays full-graph construction cost per insert.

**Pure search, zero writers:**

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 3,618 | 3,642 | 393 |
| 2 | 6,806 | 7,396 | 905 |
| 4 | 13,262 | 14,478 | 1,665 |
| 8 | **26,256** | **27,050** | 3,506 |

Sharding's bill comes due: every query pays 16 sub-searches plus a merge —
**~9× slower** at every thread count. The uncontended RwLock keeps a small
constant edge over lock-free (epoch pinning is not free); that is the
honest cost of wait-free reads.

**Mixed 90% search / 10% insert** (the realistic agent-memory workload):

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 16,039 | 22,106 | 529 |
| 2 | 38,337 | 20,335 | 1,120 |
| 4 | 66,440 | 16,298 | 2,110 |
| 8 | **114,570** | 22,621 | 5,204 |

The workload that matters, and the only one with a single winner: the
lock-free index beats the flat RwLock **5×** and the sharded design **22×**
at 8 threads. Sharding's read tax devours its write advantage the moment
searches dominate; the RwLock throttles everyone once 10% of traffic
writes.

Numbers vary with hardware and run-to-run (~±15% between sessions); the
scaling *shapes* and order-of-magnitude gaps are the durable signal. Full
method and analysis: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

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

Snapshots are a versioned binary format (`CHRONO1` magic + format byte);
the index is rebuilt on load.

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
- **Known limits**: tombstones accumulate until a snapshot reload; the
  capacity check is approximate under concurrent insertion (bounded
  overshoot); concurrent inserts cannot link to each other (standard for
  concurrent HNSW construction — covered by the stress recall gate).
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
