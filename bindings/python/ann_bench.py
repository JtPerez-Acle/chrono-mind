#!/usr/bin/env python3
"""Real-dataset ANN benchmark driver for ChronoMind.

Loads an ann-benchmarks HDF5 dataset (train / test / neighbors) and traces
the recall@k vs queries-per-second frontier for ChronoMind and any of the
competitor libraries that happen to be importable in the current
environment (usearch, hnswlib). The same script therefore runs on Windows
(chronomind + usearch) and in WSL/Linux (also hnswlib, faiss).

Recall is scored against the dataset's bundled exact ground-truth
neighbors, so no brute-force pass is needed. QPS is single-threaded,
one query at a time — the ann-benchmarks convention — so the numbers are
directly comparable to the published leaderboard curves.

Usage:
    python ann_bench.py --data data/glove-100-angular.hdf5
    python ann_bench.py --data data/glove-100-angular.hdf5 --count 100000
"""

import argparse
import time

import h5py
import numpy as np


def load(path, count=None):
    with h5py.File(path, "r") as f:
        train = np.asarray(f["train"], dtype=np.float32)
        test = np.asarray(f["test"], dtype=np.float32)
        neighbors = np.asarray(f["neighbors"], dtype=np.int64)
        dist = f.attrs.get("distance", "?")
    if count is not None:
        # Subsetting the corpus invalidates the bundled ground truth (it
        # indexes the full train set), so recompute it for the subset.
        train = train[:count]
        neighbors = None
    return train, test, neighbors, dist


def exact_neighbors(train, test, k, angular):
    """Brute-force top-k, used only when the corpus was subsetted."""
    t = train
    q = test
    if angular:
        t = t / np.linalg.norm(t, axis=1, keepdims=True)
        q = q / np.linalg.norm(q, axis=1, keepdims=True)
        sims = q @ t.T
        return np.argsort(-sims, axis=1)[:, :k]
    # euclidean
    out = np.empty((len(q), k), dtype=np.int64)
    for i in range(len(q)):
        d = np.sum((t - q[i]) ** 2, axis=1)
        out[i] = np.argsort(d)[:k]
    return out


def recall_at_k(got, truth, k):
    hits = 0
    for g, t in zip(got, truth):
        hits += len(set(g[:k]) & set(t[:k].tolist()))
    return hits / (len(got) * k)


# ---- algorithm adapters: each builds an index and answers queries -------


class ChronoMind:
    name = "chronomind"

    def __init__(self, dim, m, efc):
        import chronomind

        self.idx = chronomind.Index(dim, max_connections=m, ef_construction=efc)

    def fit(self, X):
        self.idx.fit(X)

    def set_ef(self, ef):
        self.idx.set_ef_search(ef)

    def query(self, v, k):
        return self.idx.query(v, k)


class USearch:
    name = "usearch"

    def __init__(self, dim, m, efc):
        from usearch.index import Index

        self.idx = Index(
            ndim=dim, metric="cos", dtype="f32", connectivity=m, expansion_add=efc
        )

    def fit(self, X):
        self.idx.add(np.arange(len(X)), X)

    def set_ef(self, ef):
        self.idx.expansion_search = ef

    def query(self, v, k):
        return self.idx.search(v, k).keys


class HnswLib:
    name = "hnswlib"

    def __init__(self, dim, m, efc):
        import hnswlib

        self.idx = hnswlib.Index(space="cosine", dim=dim)
        self._m, self._efc = m, efc

    def fit(self, X):
        self.idx.init_index(max_elements=len(X), ef_construction=self._efc, M=self._m)
        self.idx.add_items(X, np.arange(len(X)))

    def set_ef(self, ef):
        self.idx.set_ef(ef)

    def query(self, v, k):
        labels, _ = self.idx.knn_query(v, k=k)
        return labels[0]


class Faiss:
    name = "faiss-hnsw"

    def __init__(self, dim, m, efc):
        import faiss

        # Cosine via inner product on unit-normalized vectors (FAISS does not
        # normalize itself, so the adapter does it in fit/query).
        self.idx = faiss.IndexHNSWFlat(dim, m, faiss.METRIC_INNER_PRODUCT)
        self.idx.hnsw.efConstruction = efc

    def fit(self, X):
        xn = X / np.linalg.norm(X, axis=1, keepdims=True)
        self.idx.add(np.ascontiguousarray(xn, dtype=np.float32))

    def set_ef(self, ef):
        self.idx.hnsw.efSearch = ef

    def query(self, v, k):
        vn = (v / np.linalg.norm(v)).reshape(1, -1).astype(np.float32)
        _, ids = self.idx.search(vn, k)
        return ids[0]


def available():
    algos = [ChronoMind]
    for mod, cls in (("usearch", USearch), ("hnswlib", HnswLib), ("faiss", Faiss)):
        try:
            __import__(mod)
            algos.append(cls)
        except ImportError:
            pass
    return algos


def bench(algo_cls, train, test, truth, k, m, efc, ef_sweep):
    dim = train.shape[1]
    algo = algo_cls(dim, m, efc)
    t0 = time.perf_counter()
    algo.fit(train)
    build = time.perf_counter() - t0

    rows = []
    for ef in ef_sweep:
        if ef < k:
            continue
        algo.set_ef(ef)
        got = []
        t0 = time.perf_counter()
        for q in test:
            got.append(np.asarray(algo.query(q, k)))
        elapsed = time.perf_counter() - t0
        qps = len(test) / elapsed
        rec = recall_at_k(got, truth, k)
        rows.append((ef, rec, qps))
    return build, rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--k", type=int, default=10)
    ap.add_argument("--m", type=int, default=16)
    ap.add_argument("--ef-construction", type=int, default=100)
    ap.add_argument(
        "--ef-search",
        type=int,
        nargs="+",
        default=[10, 20, 40, 80, 120, 200, 400],
    )
    ap.add_argument("--count", type=int, default=None, help="subset corpus size")
    args = ap.parse_args()

    train, test, truth, dist = load(args.data, args.count)
    angular = "angular" in str(dist) or "cos" in str(dist)
    if truth is None:
        print(f"recomputing ground truth for subset n={len(train)} ...", flush=True)
        truth = exact_neighbors(train, test, args.k, angular)

    print(
        f"dataset={args.data} n={len(train)} dim={train.shape[1]} "
        f"queries={len(test)} distance={dist} k={args.k} "
        f"M={args.m} efC={args.ef_construction}"
    )

    for algo_cls in available():
        print(f"\n## {algo_cls.name}", flush=True)
        build, rows = bench(
            algo_cls, train, test, truth, args.k, args.m, args.ef_construction,
            args.ef_search,
        )
        print(f"build: {build:.1f}s ({len(train)/build:,.0f} ins/s, 1 thread)")
        print(f"| efSearch | recall@{args.k} | QPS (1T) |")
        print("|---:|---:|---:|")
        for ef, rec, qps in rows:
            print(f"| {ef} | {rec:.4f} | {qps:,.0f} |")


if __name__ == "__main__":
    main()
