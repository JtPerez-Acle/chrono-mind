# GloVe-100-angular — real-data frontier

Dataset: full ann-benchmarks glove-100-angular (n=1,183,514, dim=100, 10k
queries), exact bundled ground truth. Params M=16, efConstruction=100, f32,
cosine. QPS is one-query-at-a-time, single thread (the ann-benchmarks
convention). Driver: `ann_bench.py --data data/glove-100-angular.hdf5`.

## Headline (three-way, one run, WSL2 Ubuntu / i7-12700KF)

chronomind vs usearch 2.25 (SIMD C++) vs FAISS 1.14 IndexHNSWFlat (Meta),
all in a single process so the machine state is identical and the systems
are directly comparable.

> Note: chronomind beats FAISS on *this* dataset, but FAISS is dataset-
> dependent — on NYTimes-256 it is ~2× the fastest. See
> [nytimes-256-angular.md](nytimes-256-angular.md) for the tempered
> cross-library picture. The durable claim is parity with usearch, not
> beating the whole field.

| efSearch | chrono recall / QPS | usearch recall / QPS | faiss recall / QPS |
|---:|---|---|---|
| 10  | **0.4702** / 7,248 | 0.4532 / **8,664** | 0.4596 / 7,107 |
| 20  | **0.5872** / 4,598 | 0.5746 / **5,744** | 0.5788 / 4,238 |
| 40  | **0.6931** / 2,845 | 0.6811 / **3,648** | 0.6839 / 2,504 |
| 80  | **0.7760** / 1,651 | 0.7680 / **2,168** | 0.7667 / 1,215 |
| 120 | **0.8154** / 1,203 | 0.8096 / **1,552** | 0.8062 / 879 |
| 200 | **0.8576** / 784   | 0.8515 / **1,065** | 0.8482 / 517 |
| 400 | **0.9015** / 419   | 0.8970 / **588**   | 0.8935 / 252 |

Build (single thread):

| system | build | ins/s |
|---|---:|---:|
| chronomind | 819.4s | 1,444 |
| usearch | 60.1s | 19,696 |
| faiss-hnsw | 70.8s | 16,710 |

## Reading it

- **Recall: chronomind is highest at every efSearch.** Its graph quality
  matches or slightly beats both references.
- **Search QPS: chronomind beats FAISS at every point** (decisively at the
  high-recall end: 419 vs 252 QPS at ef=400, +66%) and is **at parity with
  usearch** — usearch edges it in this Linux run, chronomind edged usearch
  in a Windows run; the lead alternates within noise. chronomind already has
  an AVX2+FMA distance kernel and at 1.18M nodes search is memory-latency-
  bound, so it is not at a SIMD disadvantage.
- **Build: chronomind is ~12-14x slower** than both. This is structural —
  lock-free construction overhead (COW neighbor lists, epoch pinning, CAS
  retries, per-insert allocation), not the distance metric (see the
  optimization note below).

## Caveats (stated plainly)

- **Single-threaded throughout.** This does NOT exercise chronomind's actual
  differentiator — concurrent build and wait-free reads under concurrent
  writers. usearch and faiss are also single-threaded here. On the axis
  chronomind is built for, there is no standard single-process benchmark;
  this is the worst-case framing for it and it still ties/leads on search.
- **FAISS adapter pays a small per-query Python tax** (it normalizes each
  query in numpy, since FAISS has no internal cosine; chronomind and usearch
  normalize in compiled code). This slightly understates FAISS QPS — but by
  far less than the 66% gap at ef=400. FAISS is also tuned for *batch*
  search; single-query is its weak mode.
- One dataset, one machine, f32 (no usearch/faiss quantization).
- hnswlib omitted: it requires python3-dev to compile and sudo was not
  available in this environment; FAISS serves as the reference HNSW.

## Optimization note: normalize-once underdelivered on x86

Storing unit-normalized vectors and using a bare dot product (vs recomputing
both operands' norms per call) was expected to close much of the build gap.
It did not. chronomind's distance was *already* AVX2+FMA, and that kernel is
memory-bound: it loads the same two vectors per iteration regardless of
whether it computes one FMA (dot) or three (dot + two norms). Removing the
norm FMAs barely helps when memory loads are the bottleneck. The build gap
is therefore lock-free construction overhead, not distance arithmetic. The
change is kept anyway: principled (matches usearch/faiss/hnswlib), regresses
nothing (all gates green; search unchanged), and helps more on non-AVX2
targets (e.g. aarch64) where the scalar kernel is compute-bound.

## Cross-run variance note

Absolute numbers are not comparable across separate runs (back-to-back
Windows runs showed usearch — unchanged code — swing ~40% on QPS from
thermal throttling). Compare systems only within a single run. The two
qualitative conclusions (search parity/lead; build many-x slower) are stable
across all runs.
