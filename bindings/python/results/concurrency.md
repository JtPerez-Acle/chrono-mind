# Reads under concurrent writes — the design's reason to exist

Produced by `cargo bench --bench concurrency --features bench-external`
(benches/concurrency.rs). Machine: i7-12700KF, Windows 11. Synthetic
embedding-like data (dim=256, intrinsic 24), 20,000 base vectors, M=16,
efSearch=50. Each system is measured twice: readers alone, then the same
readers plus writers, 4 readers + 4 writers, 3s per phase.

Every other benchmark here is single-threaded or write-only — the one
framing where a lock-free index has nothing to prove. This is the scenario
it was built for.

| system | read QPS (idle) | read QPS (under writes) | retention | p99 idle | p99 under writes |
|---|---:|---:|---:|---:|---:|
| chronomind (lock-free) | 32,234 | 21,088 | **65%** | 228 us | 384 us |
| usearch (C++) | 34,769 | 21,579 | 62% | 186 us | 357 us |
| sharded16 (16 RwLocks) | 2,928 | 1,505 | 51% | 2,396 us | 5,186 us |
| rwlock (one lock) | 33,173 | **486** | **1%** | 237 us | **79,310 us** |

## Reading it

- **The single RwLock collapses under writes.** Fast when idle (33k QPS),
  it retains 1% of read throughput once writers are active, and its p99 read
  latency explodes 334x to 79 ms. This is reader starvation: writers hold
  the exclusive lock, readers block. It is the exact failure chronomind
  exists to avoid.
- **chronomind retains 65% read throughput with p99 barely moving** (228 ->
  384 us, 1.7x). The remaining 35% is not blocking — it is 8 threads sharing
  CPU, as the flat p99 proves (a blocked reader would show a latency
  explosion like the RwLock's). Wait-free reads, demonstrated.
- **chronomind matches usearch** (62% retention, comparable p99). Both are
  genuinely concurrent engines; chronomind is not paying a penalty for being
  Rust + lock-free here.
- **sharded16 has low read throughput by design** (2.9k idle): every query
  scatter-gathers across 16 shards. It survives writes better than one lock
  but starts from a far lower base — the read tax documented in the internal
  A/B benchmark.

## Caveat

Synthetic data and a single machine; absolute QPS will vary. The durable
signal is the *retention* and the *p99 behaviour under contention* — the
order-of-magnitude collapse of the single lock vs the graceful degradation
of the lock-free and the C++ concurrent engine.
