//! PyO3 bindings exposing the ChronoMind lock-free HNSW index to Python,
//! for head-to-head benchmarking against other ANN libraries on the
//! standard datasets (SIFT / GloVe / GIST in HDF5 form).
//!
//! The surface mirrors the de-facto ann-benchmarks contract so the same
//! object can drop into that harness or our own:
//!   - `Index(dim, max_connections, ef_construction, ef_search, seed)`
//!   - `fit(data)`            build from an (n, dim) float32 array
//!   - `set_ef_search(ef)`    sweep the query-time accuracy knob
//!   - `query(v, n)`          return the handles of the n nearest neighbors
//!   - `batch_query(qs, n)`   one call for an (m, dim) block of queries
//!
//! Build is single-threaded, so handle `i` corresponds to row `i` of the
//! `fit` matrix — the handles ARE the original dataset indices, which is
//! what recall scoring expects.

use std::sync::Arc;

use numpy::{PyReadonlyArray1, PyReadonlyArray2};
use pyo3::prelude::*;

// `::chronomind` disambiguates the dependency crate from this module, which
// must also be named `chronomind` so Python can `import chronomind`.
use ::chronomind::config::IndexParams;
use ::chronomind::index::{LockFreeHnsw, VectorIndex};
use ::chronomind::metric::CosineDistance;

/// A ChronoMind lock-free HNSW index over unit-norm (cosine) vectors.
#[pyclass]
struct Index {
    inner: LockFreeHnsw,
    ef_search: usize,
}

#[pymethods]
impl Index {
    #[new]
    #[pyo3(signature = (dim, max_connections = 16, ef_construction = 100, ef_search = 50, seed = 0x5EED))]
    fn new(
        dim: usize,
        max_connections: usize,
        ef_construction: usize,
        ef_search: usize,
        seed: u64,
    ) -> Self {
        // `dim` is implied by the vectors handed to `fit`/`query`; it is
        // accepted only so the constructor matches the conventional
        // ann-benchmarks wrapper signature.
        let _ = dim;
        let params = IndexParams {
            max_connections,
            ef_construction,
            ef_search,
        };
        let metric = Arc::new(CosineDistance::new());
        Index {
            inner: LockFreeHnsw::with_seed(params, metric, seed),
            ef_search,
        }
    }

    /// Build the index from an (n, dim) float32 array. Row `i` is inserted
    /// as handle `i`.
    fn fit(&mut self, data: PyReadonlyArray2<'_, f32>) {
        let arr = data.as_array();
        for row in arr.outer_iter() {
            // ndarray rows are not guaranteed contiguous; materialize.
            let v: Vec<f32> = row.iter().copied().collect();
            self.inner.insert(&v);
        }
    }

    /// Set the query-time `efSearch`. Higher trades latency for recall.
    fn set_ef_search(&mut self, ef: usize) {
        self.ef_search = ef;
    }

    /// Return the handles of the `n` nearest neighbors to `v`.
    fn query(&self, v: PyReadonlyArray1<'_, f32>, n: usize) -> Vec<u32> {
        let q: Vec<f32> = v.as_array().iter().copied().collect();
        let ef = self.ef_search.max(n);
        self.inner
            .search(&q, ef)
            .into_iter()
            .take(n)
            .map(|(id, _)| id)
            .collect()
    }

    /// Run a block of queries `(m, dim)`, returning `m` lists of handles.
    /// The GIL is released for the search itself, so a caller may parallelize.
    fn batch_query(
        &self,
        py: Python<'_>,
        queries: PyReadonlyArray2<'_, f32>,
        n: usize,
    ) -> Vec<Vec<u32>> {
        let owned: Vec<Vec<f32>> = queries
            .as_array()
            .outer_iter()
            .map(|row| row.iter().copied().collect())
            .collect();
        let ef = self.ef_search.max(n);
        py.allow_threads(|| {
            owned
                .iter()
                .map(|q| {
                    self.inner
                        .search(q, ef)
                        .into_iter()
                        .take(n)
                        .map(|(id, _)| id)
                        .collect()
                })
                .collect()
        })
    }

    /// Number of live vectors.
    fn __len__(&self) -> usize {
        self.inner.len()
    }
}

#[pymodule]
fn chronomind(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Index>()?;
    Ok(())
}
