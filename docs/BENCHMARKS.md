# Benchmarks

Four comparisons, in descending order of how much they matter:

1. **Real datasets vs usearch and FAISS** — standard ann-benchmarks data
   through the Python bindings; locates us in the field on honest ground.
2. **Reads under concurrent writes** — the scenario the lock-free design
   exists for; the one axis where it wins rather than ties.
3. **Internal A/B** — lock-free vs our own locked baselines; isolates the
   concurrency-control variable.
4. **In-process synthetic head-to-head** — five libraries, one synthetic
   dataset, all in one Rust process.

Per-run tables with full caveats live in
[`bindings/python/results/`](../bindings/python/results/).

## Real datasets: vs usearch and FAISS

Through the PyO3 bindings (`bindings/python/`, driver `ann_bench.py`),
chronomind, usearch 2.25 (SIMD C++), and FAISS 1.14 (`IndexHNSWFlat`, Meta)
run over standard ann-benchmarks datasets with their exact bundled ground
truth. M=16, efC=100, f32, cosine. QPS is one query at a time, single
thread — the ann-benchmarks convention. Each dataset's three systems run in
one process, so machine state is shared and comparable.

**Recall@10 vs single-thread QPS, at ef=400 (the high-recall end):**

| dataset | chrono recall / QPS | usearch recall / QPS | faiss recall / QPS |
|---|---|---|---|
| GloVe-100 (1.18M, 100-d) | 0.9015 / 419 | 0.8970 / 588 | 0.8935 / 252 |
| NYTimes-256 (290k, 256-d) | 0.9008 / 613 | 0.9049 / 681 | 0.8943 / 1,353 |

**Single-thread build:**

| dataset | chronomind | usearch | faiss |
|---|---:|---:|---:|
| GloVe-100 | 819s | 60s | 71s |
| NYTimes-256 | 155s | 13s | 9s |

Reading it honestly:

- **Recall matches the field on both** — all three within ~0.01 at every ef.
- **Search throughput tracks usearch within ~10%**, with the lead
  alternating (chronomind ahead on GloVe-100, ~10% behind on NYTimes-256).
  chronomind already uses an AVX2+FMA kernel, so it is not SIMD-limited;
  at million scale search is memory-latency-bound.
- **FAISS is dataset-dependent** — slowest of the three on GloVe-100, ~2×
  the fastest on NYTimes-256. So the honest cross-library claim is
  "competitive with usearch," not "beats the field"; a single dataset
  (GloVe) overstated it, which is exactly why the second dataset was run.
- **Build is 12–24× slower** than both — structural lock-free construction
  overhead (COW lists, epoch pinning, CAS, per-insert allocation), not the
  distance metric. This is the weakest number and is not hidden.

Caveats: single-threaded throughout (does NOT exercise the concurrent
design point — see the next section); one machine; f32, no quantization;
NYTimes contains zero vectors that mildly perturb the FAISS adapter. Full
per-dataset tables: [`bindings/python/results/`](../bindings/python/results/).

## Reads under concurrent writes

`cargo bench --bench concurrency --features bench-external`. The scenario
the lock-free design exists for, and the one every other benchmark here
fails to exercise. 4 reader threads and 4 writer threads share one index
(dim=256, 20k base, M=16, efS=50); read throughput and p99 read latency are
measured with readers alone, then with writers running too.

| system | read QPS idle | read QPS under writes | retention | p99 idle | p99 under writes |
|---|---:|---:|---:|---:|---:|
| chronomind (lock-free) | 32,234 | 21,088 | **65%** | 228 µs | 384 µs |
| usearch (C++) | 34,769 | 21,579 | 62% | 186 µs | 357 µs |
| sharded16 | 2,928 | 1,505 | 51% | 2,396 µs | 5,186 µs |
| rwlock (one lock) | 33,173 | 486 | **1%** | 237 µs | **79,310 µs** |

- **The single RwLock collapses**: 1% read retention, p99 explodes 334× to
  79 ms. Writers hold the exclusive lock; readers starve. This is the
  failure the whole design avoids.
- **chronomind retains 65% with p99 barely moving** (1.7×). The drop is CPU
  sharing across 8 threads, not blocking — a blocked reader would show the
  RwLock's latency explosion, not a flat p99. Wait-free reads, demonstrated.
- **chronomind matches usearch** — both are genuinely concurrent; lock-free
  Rust pays no penalty here.
- sharded16 survives writes better than one lock but starts from a far lower
  read base (scatter-gather across 16 shards).

## Where we stand: in-process synthetic head-to-head

`cargo bench --bench external --features bench-external` — five systems,
identical seeded data (10,000 × 768-d embedding-like vectors, unit norm),
identical parameters (M=16, efC=100, efS=50 or each library's nearest
equivalent), f32 everywhere, recall measured against shared brute-force
ground truth. Ranges over two runs (i7-12700KF, Windows 11):

| system | build 1T | build parallel | QPS 1T | QPS 8T | recall@10 |
|---|---:|---:|---:|---:|---:|
| **chronomind (lock-free)** | 4.7–5.2s | **0.61–0.70s** | **3,393–4,231** | **29,038–33,824** | 0.998 |
| chrono-sharded16 | 2.2–2.6s | 0.30–0.34s | 393–449 | 3,487–3,526 | 1.000 |
| instant-distance 0.6 | — (bulk only) | 6.9–7.3s | 1,031–1,153 | 9,325–9,437 | 1.000 |
| hnsw_rs 0.3.4 | 14.1–14.6s | 1.27–1.36s (rayon) | 1,169–1,225 | 6,957–7,306 | 0.969–0.973 |
| usearch 2.25 (C++) | 4.0–5.0s | 0.53–0.55s | 3,093–3,965 | 28,757–30,961 | 0.997–0.998 |

Reading it honestly:

- **Search throughput is at parity with usearch** — the SIMD-heavy C++
  engine — within run-to-run noise (each run had a different leader), at
  identical recall. Both are ~3–4× faster than the pure-Rust crates.
- **Parallel build is competitive with usearch** (~20% slower) and
  ~2× faster than hnsw_rs's rayon build; our single-thread build is
  mid-pack.
- **Recall is top-tier**: 0.998 vs ground truth, above hnsw_rs (~0.97) at
  the same nominal parameters.
- Only chronomind, hnsw_rs, and usearch support incremental insert;
  instant-distance is bulk-build-only, and only chronomind and usearch
  support deletes. Among the contenders, only chronomind's reads are
  wait-free under concurrent writers.

Scope caveats, stated plainly: one dataset shape (synthetic, low intrinsic
dimension — chosen to model embeddings), one size (10k), one machine, f32
only. usearch ships i8/bf16 quantization that would shrink its memory and
beat everyone's f32 throughput — disabled here for apples-to-apples. This
is not an ann-benchmarks run on SIFT/GIST at million scale; treat it as a
strong local signal, not a leaderboard entry.

## Internal A/B: lock-free vs our own locked baselines

A/B comparison of the lock-free HNSW index (`LockFreeHnsw`) against the
RwLock baseline (`RwLockHnsw`) — same algorithm, same parameters, different
concurrency control.

### Method

- `cargo bench` (criterion 0.5), bench target `benches/comparison.rs`.
- Data: 768-dimensional unit vectors confined to ONE random 16-d subspace
  ("embedding-like" — real embeddings have low intrinsic dimensionality;
  uniform 768-d noise is not a representative workload). Corpus, queries,
  and insert streams all share that subspace — as if produced by one
  embedding model — so a query has genuine near-neighbors in the corpus.
- Index parameters: M = 16, efConstruction = 100, efSearch = 50.
- Workloads, each at 1 / 2 / 4 / 8 threads:
  - `insert_throughput`: build a 4,000-vector index from scratch, work
    split evenly across threads.
  - `search_qps`: 2,000 queries against a pre-built 10,000-vector index.
  - `mixed_90_10`: 2,000 ops, 90% search / 10% insert, against a
    pre-built 10,000-vector index, one index per thread-count config. The
    10% inserts accumulate across criterion samples, but because every query
    has near-neighbors in the original corpus (shared subspace), search cost
    does not depend on how many inserts a given sample happened to run. An
    earlier version got this wrong: queries lived in a subspace *foreign* to
    the corpus, so searches only found close points among the accumulated
    inserts — making throughput balloon with sample count and scale
    non-monotonically. Sharing the subspace fixed it.
- Sizes are deliberately minutes-scale: the meaningful signal is the
  *relative scaling under contention* of the implementations, which is
  stable across corpus size; absolute numbers vary with hardware. (These
  criterion measurements run a touch lower than the external suite's
  best-of-three QPS — e.g. ~26K vs ~29K search at 8T — because criterion
  reports a full-distribution estimate, not the best pass.)

### Contenders

- `lockfree` — the lock-free index (`LockFreeHnsw`)
- `rwlock` — the same algorithm behind one `RwLock` (`RwLockHnsw`)
- `sharded16` — the *fair* baseline (`ShardedRwLockHnsw`): 16 independently
  locked shards, round-robin inserts, scatter-gather search. This is what a
  practitioner would actually deploy to make a locked design scale writes.

### Results

Machine: Intel i7-12700KF (12 cores / 20 threads), Windows 11, Rust stable
1.93, June 2026. Ops/sec (criterion mid-estimates), all three columns from
one coherent run.

### insert_throughput — 4,000 concurrent inserts

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 3,859 | 4,123 | 10,130 |
| 2 | 7,839 | 4,034 | 20,204 |
| 4 | 15,286 | 4,028 | 39,264 |
| 8 | 28,300 | 4,038 | 71,152 |

### search_qps — 2,000 queries, 10,000-vector index, no writers

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 3,539 | 3,585 | 387 |
| 2 | 7,051 | 7,305 | 1,088 |
| 4 | 14,075 | 14,673 | 2,483 |
| 8 | 26,834 | 27,904 | 5,008 |

### mixed_90_10 — 2,000 ops, 90% search / 10% insert

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 4,922 | 5,582 | 567 |
| 2 | 11,420 | 5,558 | 1,138 |
| 4 | 28,733 | 5,721 | 2,325 |
| 8 | 59,998 | 10,858 | 4,012 |

Scaling is monotonic for every contender (it was not, before the subspace
fix above). Absolute numbers drift with machine state across sessions; the
scaling *shapes* and the order-of-magnitude gaps are the durable signal.

### Reading the three-way comparison

- **Sharding wins pure construction, honestly.** Round-robin over 16
  shards means each insert searches a graph 1/16th the size *and* writers
  rarely contend — 2.5× over lock-free at 8 threads. If your workload is
  bulk-load-then-freeze, shard it.
- **Sharding loses everything else.** Every query pays 16 sub-searches
  plus a merge: 9× slower on pure search at 1 thread (~5× at 8, where its
  shards scale but from a deep hole), and 15× slower on the mixed workload
  at 8 threads. The read tax is structural, not tunable.
- **The single RwLock is flat wherever writes exist** — insert throughput
  is flat at ~4k across all thread counts (one writer at a time), and on the
  mixed workload its exclusive 10% inserts stall every reader, so it barely
  moves from 1→4 threads. At 1 thread, uncontended, it is marginally *ahead*
  of lock-free on both search (~2–4%) and mixed (13%): an uncontended read
  guard is a hair cheaper than epoch pinning. That edge vanishes the instant
  a second thread writes.
- **The lock-free index is the only contender that wins the realistic
  workload** (search-dominated with steady writes) — 60k ops/s at 8 threads,
  5.5× the RwLock and 15× sharded — and the only one whose read latency is
  immune to writers (wait-free; see the reads-under-writes section above).
  Its concessions: a few percent constant read overhead vs an uncontended
  RwLock, and per-insert full-graph construction cost vs sharding's smaller
  graphs.

