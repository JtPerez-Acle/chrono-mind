//! Distance metrics for vector comparison.
//!
//! The built-in [`CosineDistance`] uses AVX2+FMA SIMD on `x86_64` when the
//! CPU supports it, with a portable scalar fallback everywhere else.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// A distance/similarity metric over `f32` vectors.
///
/// Implementations must be cheap to call: the index invokes
/// [`distance`](DistanceMetric::distance) once per visited graph edge.
pub trait DistanceMetric: Send + Sync {
    /// Distance between two vectors. For cosine this is `1 - cos(a, b)`,
    /// ranging over `[0.0, 2.0]` (0 = identical direction).
    ///
    /// Mismatched lengths and zero vectors yield the maximum distance rather
    /// than panicking, so a corrupt query degrades instead of crashing.
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;

    /// Cosine-style similarity in `[-1.0, 1.0]` (higher = more similar).
    fn similarity(&self, a: &[f32], b: &[f32]) -> f32;

    /// Short identifier for diagnostics.
    fn name(&self) -> &'static str;
}

/// Cosine distance with SIMD acceleration.
#[derive(Debug, Clone, Copy, Default)]
pub struct CosineDistance;

impl CosineDistance {
    /// Create a new instance.
    pub fn new() -> Self {
        Self
    }

    /// Dot product, squared norm of `a`, and squared norm of `b` in one pass.
    fn dot_and_norms_scalar(a: &[f32], b: &[f32]) -> (f32, f32, f32) {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;
        for (x, y) in a.iter().zip(b.iter()) {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }
        (dot, norm_a, norm_b)
    }

    /// AVX2+FMA single-pass dot product and squared norms.
    ///
    /// # Safety
    /// Caller must ensure the CPU supports AVX2 and FMA (checked via
    /// `is_x86_feature_detected!` at the call site) and `a.len() == b.len()`.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn dot_and_norms_avx2(a: &[f32], b: &[f32]) -> (f32, f32, f32) {
        let mut dot = _mm256_setzero_ps();
        let mut na = _mm256_setzero_ps();
        let mut nb = _mm256_setzero_ps();
        let chunks = a.len() / 8 * 8;

        for i in (0..chunks).step_by(8) {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            dot = _mm256_fmadd_ps(va, vb, dot);
            na = _mm256_fmadd_ps(va, va, na);
            nb = _mm256_fmadd_ps(vb, vb, nb);
        }

        #[inline]
        unsafe fn hsum(v: __m256) -> f32 {
            let lo = _mm256_castps256_ps128(v);
            let hi = _mm256_extractf128_ps(v, 1);
            let sum128 = _mm_add_ps(lo, hi);
            let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
            let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
            _mm_cvtss_f32(sum32)
        }

        let mut dot_s = hsum(dot);
        let mut na_s = hsum(na);
        let mut nb_s = hsum(nb);

        for i in chunks..a.len() {
            let (x, y) = (*a.get_unchecked(i), *b.get_unchecked(i));
            dot_s += x * y;
            na_s += x * x;
            nb_s += y * y;
        }

        (dot_s, na_s, nb_s)
    }

    fn cosine(a: &[f32], b: &[f32]) -> Option<f32> {
        if a.is_empty() || a.len() != b.len() {
            return None;
        }

        #[cfg(target_arch = "x86_64")]
        let (dot, norm_a, norm_b) = {
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                // SAFETY: feature support verified on the line above; lengths
                // verified equal at the top of the function.
                unsafe { Self::dot_and_norms_avx2(a, b) }
            } else {
                Self::dot_and_norms_scalar(a, b)
            }
        };
        #[cfg(not(target_arch = "x86_64"))]
        let (dot, norm_a, norm_b) = Self::dot_and_norms_scalar(a, b);

        let denom = (norm_a * norm_b).sqrt();
        if denom <= f32::EPSILON {
            return None; // zero vector: similarity undefined
        }
        Some((dot / denom).clamp(-1.0, 1.0))
    }
}

impl DistanceMetric for CosineDistance {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match Self::cosine(a, b) {
            Some(cos) => 1.0 - cos,
            None => 2.0,
        }
    }

    fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        Self::cosine(a, b).unwrap_or(0.0)
    }

    fn name(&self) -> &'static str {
        "cosine"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    #[test]
    fn identical_vectors_have_zero_distance() {
        let m = CosineDistance::new();
        let v = vec![1.0, 0.5, -0.25, 2.0];
        assert!(m.distance(&v, &v).abs() < EPS);
        assert!((m.similarity(&v, &v) - 1.0).abs() < EPS);
    }

    #[test]
    fn orthogonal_vectors_have_unit_distance() {
        let m = CosineDistance::new();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((m.distance(&a, &b) - 1.0).abs() < EPS);
        assert!(m.similarity(&a, &b).abs() < EPS);
    }

    #[test]
    fn opposite_vectors_have_max_distance() {
        let m = CosineDistance::new();
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        assert!((m.distance(&a, &b) - 2.0).abs() < EPS);
    }

    #[test]
    fn degenerate_inputs_yield_max_distance() {
        let m = CosineDistance::new();
        assert_eq!(m.distance(&[], &[]), 2.0);
        assert_eq!(m.distance(&[1.0], &[1.0, 2.0]), 2.0);
        assert_eq!(m.distance(&[0.0, 0.0], &[0.0, 0.0]), 2.0);
        assert_eq!(m.similarity(&[0.0, 0.0], &[0.0, 0.0]), 0.0);
    }

    #[test]
    fn simd_and_scalar_paths_agree() {
        // Exercise lengths around the 8-lane SIMD boundary, including the
        // scalar remainder path.
        let m = CosineDistance::new();
        for len in [1usize, 7, 8, 9, 15, 16, 17, 768] {
            let a: Vec<f32> = (0..len).map(|i| ((i * 37 % 19) as f32) - 9.0).collect();
            let b: Vec<f32> = (0..len).map(|i| ((i * 53 % 23) as f32) - 11.0).collect();
            let (dot, na, nb) = CosineDistance::dot_and_norms_scalar(&a, &b);
            let denom = (na * nb).sqrt();
            if denom <= f32::EPSILON {
                continue;
            }
            let expected = 1.0 - (dot / denom).clamp(-1.0, 1.0);
            let got = m.distance(&a, &b);
            assert!(
                (got - expected).abs() < 1e-4,
                "len {len}: simd {got} vs scalar {expected}"
            );
        }
    }
}
