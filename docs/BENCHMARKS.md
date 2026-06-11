# Benchmarks

A/B comparison of the lock-free HNSW index (`LockFreeHnsw`) against the
RwLock baseline (`RwLockHnsw`) — same algorithm, same parameters, different
concurrency control.

## Method

- `cargo bench` (criterion 0.5), bench target `benches/comparison.rs`.
- Data: 768-dimensional unit vectors confined to a random 16-d subspace
  ("embedding-like" — real embeddings have low intrinsic dimensionality;
  uniform 768-d noise is not a representative workload).
- Index parameters: M = 16, efConstruction = 100, efSearch = 50.
- Workloads, each at 1 / 2 / 4 / 8 threads:
  - `insert_throughput`: build a 4,000-vector index from scratch, work
    split evenly across threads.
  - `search_qps`: 2,000 queries against a pre-built 10,000-vector index.
  - `mixed_90_10`: 2,000 ops, 90% search / 10% insert, against a
    pre-built 10,000-vector index (rebuilt per sample so insert drift
    cannot compound).
- Sizes are deliberately minutes-scale: the meaningful signal is the
  *relative scaling under contention* of the two implementations, which is
  stable across corpus size; absolute numbers vary with hardware.

## Results

Machine: Intel i7-12700KF (12 cores / 20 threads), Windows 11, Rust stable
1.93, June 2026. Ops/sec (criterion mid-estimates of throughput).

### insert_throughput — 4,000 concurrent inserts

| threads | lock-free | RwLock | speedup |
|--------:|----------:|-------:|--------:|
| 1 | 3,116 | 3,352 | 0.93× |
| 2 | 6,415 | 3,303 | 1.94× |
| 4 | 12,573 | 3,280 | 3.83× |
| 8 | 21,313 | 3,277 | 6.50× |

### search_qps — 2,000 queries, 10,000-vector index, no writers

| threads | lock-free | RwLock | speedup |
|--------:|----------:|-------:|--------:|
| 1 | 4,372 | 4,671 | 0.94× |
| 2 | 9,007 | 9,470 | 0.95× |
| 4 | 17,616 | 18,395 | 0.96× |
| 8 | 29,525 | 31,037 | 0.95× |

### mixed_90_10 — 2,000 ops, 90% search / 10% insert

| threads | lock-free | RwLock | speedup |
|--------:|----------:|-------:|--------:|
| 1 | 23,619 | 27,296 | 0.87× |
| 2 | 50,165 | 25,844 | 1.94× |
| 4 | 90,918 | 26,080 | 3.49× |
| 8 | 139,150 | 32,454 | 4.29× |

Results were reproduced in a second `cargo bench` run with agreeing
ordering and scaling shape. Raw criterion reports land in
`target/criterion/`.

## Reading the numbers

- **Single-threaded, the baseline wins by 7–13%.** Same algorithm; an
  uncontended RwLock is nearly free, while the lock-free version pays for
  epoch pinning and COW slice allocation on every write. This is the
  honest constant cost of the design.
- **Any writes + any concurrency, the lock-free index wins big and the
  baseline goes flat.** Inserts: 6.5× at 8 threads, with near-linear
  lock-free scaling against a *completely flat* RwLock curve — serialized
  writers cannot use added threads. Even 10% writers (`mixed_90_10`)
  caps the baseline at roughly its single-thread throughput, because
  every exclusive writer stalls all readers.
- **Pure reads with zero writers are the RwLock's best case** — shared
  read locks admit full parallelism, and it keeps its ~5% constant-factor
  edge at every thread count. The lock-free advantage on reads is not
  raw speed but *immunity*: its read latency is unaffected by writers
  (wait-free), while the baseline's collapses the moment writers appear —
  visible in `mixed_90_10`.
