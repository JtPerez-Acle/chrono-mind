# GloVe-100-angular — real-data frontier (single thread)

Machine: Intel i7-12700KF, Windows 11. Dataset: full ann-benchmarks
glove-100-angular (n=1,183,514, dim=100, 10k queries), exact bundled ground
truth. Params M=16, efConstruction=100, f32, cosine. QPS is one-query-at-a-
time, single thread (ann-benchmarks convention). Produced by
`ann_bench.py --data data/glove-100-angular.hdf5`.

## Headline

- **Search is at parity with usearch** (the SIMD C++ engine) on real 1.18M-
  vector embedding data, at matched recall. Across two runs the QPS leader
  alternated within noise; chronomind's recall was marginally higher at
  every efSearch (slightly better graph quality).
- **Single-threaded build is ~15-24x slower than usearch.** This is
  structural (lock-free construction overhead), not the distance kernel —
  see the optimization note below. Single-threaded build is also not
  chronomind's design point: it is built for concurrent build and wait-free
  reads under writers, which a single-process ann-benchmarks run does not
  exercise.

## Measurement caveat: cross-run variance

Absolute numbers are NOT comparable across runs. usearch (unchanged code)
moved a lot between back-to-back runs — build 35.0s -> 48.1s, QPS@ef10
11,739 -> 7,173 — almost certainly thermal throttling. Compare systems only
*within* a single run (same machine state). The two within-run conclusions
(search parity; build many-x slower) are stable across both runs.

## Run with the normalize-once optimization (current code)

| system | build | ins/s | recall@10 (ef=400) |
|---|---:|---:|---:|
| chronomind | 745.7s | 1,587 | 0.9015 |
| usearch 2.25 | 48.1s | 24,589 | 0.8962 |

Search frontier (same run, same machine state):

| efSearch | chrono recall | chrono QPS | usearch recall | usearch QPS |
|---:|---:|---:|---:|---:|
| 10 | 0.4702 | 8,718 | 0.4525 | 7,173 |
| 20 | 0.5872 | 5,430 | 0.5744 | 4,661 |
| 40 | 0.6931 | 3,097 | 0.6815 | 2,922 |
| 80 | 0.7760 | 1,978 | 0.7676 | 1,715 |
| 120 | 0.8154 | 1,371 | 0.8076 | 1,250 |
| 200 | 0.8576 | 857 | 0.8500 | 815 |
| 400 | 0.9015 | 466 | 0.8962 | 435 |

## Optimization note: normalize-once underdelivered on x86

Storing unit-normalized vectors and using a bare dot product (vs recomputing
both operands' norms per call) was expected to close much of the build gap.
It did not: build improved only modestly. The reason is that chronomind's
distance was *already* AVX2+FMA, and that kernel is memory-bound — it loads
the same two vectors per iteration whether it computes one FMA (dot) or
three (dot + two norms). Removing the norm FMAs barely helps when loads are
the bottleneck. The build gap is therefore dominated by lock-free
construction overhead (COW neighbor lists, epoch pinning, CAS retries,
per-insert allocation), not by distance arithmetic.

The change is kept regardless: it is principled (what usearch/hnswlib do),
regresses nothing (all gates green, search unchanged), and helps more on
non-AVX2 targets (e.g. aarch64), where the scalar kernel is compute-bound
and dropping two-thirds of the per-element work is a real saving.

## Prior run (pre-optimization code), for reference

chronomind build 844.6s; usearch build 35.0s. Search frontier within that
run was likewise at parity (usearch marginally ahead on QPS that run).
