# NYTimes-256-angular — real-data frontier (second dataset)

Dataset: full ann-benchmarks nytimes-256-angular (n=290,000, dim=256, 10k
queries), exact bundled ground truth. Params M=16, efConstruction=100, f32,
cosine. QPS is one-query-at-a-time, single thread. Run in one process in
WSL2 Ubuntu (i7-12700KF), so the three systems share machine state.

This is the second dataset, run specifically to test whether the GloVe-100
result generalizes. It does for recall and for the chronomind/usearch
relationship — and it usefully tempers the cross-library claim.

## Frontier

| efSearch | chrono recall / QPS | usearch recall / QPS | faiss recall / QPS |
|---:|---|---|---|
| 10  | 0.5320 / 11,060 | 0.5473 / 10,921 | 0.5599 / 20,122 |
| 20  | 0.6508 / 6,811  | 0.6678 / 7,121  | 0.6703 / 14,018 |
| 40  | 0.7459 / 4,109  | 0.7600 / 4,566  | 0.7528 / 9,073 |
| 80  | 0.8138 / 2,495  | 0.8226 / 2,740  | 0.8121 / 5,588 |
| 120 | 0.8414 / 1,773  | 0.8481 / 2,002  | 0.8372 / 4,145 |
| 200 | 0.8692 / 1,147  | 0.8749 / 1,336  | 0.8649 / 2,686 |
| 400 | 0.9008 / 613    | 0.9049 / 681    | 0.8943 / 1,353 |

Build (single thread): chronomind 155.2s (1,869 ins/s); usearch 12.9s
(22,505 ins/s); faiss 9.0s (32,224 ins/s).

## Reading it (vs GloVe-100)

- **Recall: all three within ~0.01** at every ef, on both datasets.
  chronomind sits a hair below usearch and a hair above faiss here.
- **chronomind tracks usearch within ~10%** on search QPS (613 vs 681 at
  ef=400; ~10% behind). On GloVe-100 chronomind was slightly *ahead* of
  usearch. So across the two datasets the chronomind/usearch relationship is
  "parity, within ~10%, lead alternates" — the durable claim.
- **FAISS is dataset-dependent and swings hard**: slowest of the three on
  GloVe-100 (252 QPS at ef=400), ~2x the fastest on NYTimes-256 (1,353).
  This is why the honest cross-library claim is "competitive with usearch,"
  NOT "beats the field" — a single dataset (GloVe) overstated it.
- **Build: chronomind ~12-17x slower** here too (structural lock-free
  construction overhead), consistent with GloVe-100.

## Caveats specific to NYTimes-256

- **The dataset contains zero vectors.** The FAISS adapter normalizes in
  numpy and hits divide-by-zero on those (NaN queries -> a few garbage
  results), which slightly lowers FAISS recall and slightly inflates its
  QPS. chronomind (preprocess guards zero) and usearch (cosine handled
  internally) are unaffected. The effect is small (a handful of 10k
  queries) and does not explain FAISS's 2x speed.
- FAISS adapter also pays a per-query numpy normalization tax that compiled
  cosine in chronomind/usearch does not; this understates FAISS slightly.
- Single-threaded throughout — does not exercise chronomind's concurrent
  design point (see the reads-under-writes concurrency benchmark).
- One machine, f32, no quantization.
