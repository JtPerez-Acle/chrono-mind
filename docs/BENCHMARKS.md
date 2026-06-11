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

## Contenders

- `lockfree` — the lock-free index (`LockFreeHnsw`)
- `rwlock` — the same algorithm behind one `RwLock` (`RwLockHnsw`)
- `sharded16` — the *fair* baseline (`ShardedRwLockHnsw`): 16 independently
  locked shards, round-robin inserts, scatter-gather search. This is what a
  practitioner would actually deploy to make a locked design scale writes.

## Results

Machine: Intel i7-12700KF (12 cores / 20 threads), Windows 11, Rust stable
1.93, June 2026. Ops/sec (criterion mid-estimates), all three columns from
one coherent run.

### insert_throughput — 4,000 concurrent inserts

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 2,471 | 2,498 | 7,984 |
| 2 | 4,538 | 2,751 | 14,220 |
| 4 | 9,488 | 2,775 | 23,262 |
| 8 | 18,639 | 2,853 | 44,913 |

### search_qps — 2,000 queries, 10,000-vector index, no writers

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 3,618 | 3,642 | 393 |
| 2 | 6,806 | 7,396 | 905 |
| 4 | 13,262 | 14,478 | 1,665 |
| 8 | 26,256 | 27,050 | 3,506 |

### mixed_90_10 — 2,000 ops, 90% search / 10% insert

| threads | lock-free | RwLock | sharded16 |
|--------:|----------:|-------:|----------:|
| 1 | 16,039 | 22,106 | 529 |
| 2 | 38,337 | 20,335 | 1,120 |
| 4 | 66,440 | 16,298 | 2,110 |
| 8 | 114,570 | 22,621 | 5,204 |

The lockfree/rwlock shapes were reproduced across three separate `cargo
bench` sessions (absolute numbers drift ~±15% with machine state; ordering
and scaling shape agree). The sharded16 deltas are order-of-magnitude —
far beyond session variance.

## Reading the three-way comparison

- **Sharding wins pure construction, honestly.** Round-robin over 16
  shards means each insert searches a graph 1/16th the size *and* writers
  rarely contend — 2.4× over lock-free at 8 threads. If your workload is
  bulk-load-then-freeze, shard it.
- **Sharding loses everything else.** Every query pays 16 sub-searches
  plus a merge: ~9× slower on pure search, 22× slower on the mixed
  workload. The read tax is structural, not tunable.
- **The single RwLock is flat wherever writes exist** — serialized
  writers can't use threads, and exclusive writers stall every reader the
  moment 10% of traffic writes.
- **The lock-free index is the only contender that wins the realistic
  workload** (search-dominated with steady writes), and the only one whose
  read latency is immune to writers (wait-free). Its concessions: ~5%
  constant read overhead vs an uncontended RwLock, and per-insert
  full-graph construction cost vs sharding's smaller graphs.

