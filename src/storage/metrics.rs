#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Trait for implementing distance/similarity metrics
#[async_trait::async_trait]
pub trait DistanceMetric: Send + Sync {
    /// Calculate distance between two vectors
    fn calculate_distance(&self, a: &[f32], b: &[f32]) -> f32;
    /// Calculate similarity between two vectors
    fn similarity(&self, a: &[f32], b: &[f32]) -> f32;
    fn name(&self) -> &'static str;
}

/// Cosine similarity implementation
#[derive(Debug, Clone)]
pub struct CosineDistance;

impl CosineDistance {
    /// Create a new CosineDistance instance
    pub fn new() -> Self {
        Self
    }

    /// Normalize a vector to unit length
    fn normalize_vector(v: &[f32]) -> Vec<f32> {
        let magnitude = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 1e-10 {  // Use small epsilon instead of 0.0
            v.iter().map(|x| x / magnitude).collect()
        } else {
            vec![0.0; v.len()]  // Return zero vector for zero magnitude
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn dot_product_avx2(a: &[f32], b: &[f32]) -> f32 {
        let mut sum = _mm256_setzero_ps();
        let n = a.len() / 8 * 8;

        for i in (0..n).step_by(8) {
            let va = _mm256_loadu_ps(&a[i]);
            let vb = _mm256_loadu_ps(&b[i]);
            sum = _mm256_fmadd_ps(va, vb, sum);
        }

        // Horizontal sum of the 256-bit vector
        let sum128 = _mm_add_ps(
            _mm256_castps256_ps128(sum),
            _mm256_extractf128_ps(sum, 1)
        );
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        let mut result = 0.0;
        _mm_store_ss(&mut result, sum32);

        // Handle remaining elements
        for i in n..a.len() {
            result += a[i] * b[i];
        }

        result
    }
}

impl DistanceMetric for CosineDistance {
    fn calculate_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        // Handle edge cases
        if a.is_empty() || b.is_empty() || a.len() != b.len() {
            return 1.0;
        }

        // Check if either vector is zero
        let a_zero = a.iter().all(|&x| x.abs() < 1e-10);
        let b_zero = b.iter().all(|&x| x.abs() < 1e-10);
        if a_zero || b_zero {
            return 1.0;
        }

        let a_normalized = Self::normalize_vector(a);
        let b_normalized = Self::normalize_vector(b);

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                unsafe {
                    let dot_product = Self::dot_product_avx2(&a_normalized, &b_normalized);
                    // Ensure dot product is in [-1, 1] and handle numerical instability
                    let dot_product = dot_product.max(-1.0).min(1.0);
                    return (1.0 - dot_product).max(0.0);
                }
            }
        }

        // Fallback for non-x86_64 architectures or when AVX2 is not available
        let mut dot_product = 0.0;
        for (x, y) in a_normalized.iter().zip(b_normalized.iter()) {
            dot_product += x * y;
        }
        
        // Ensure dot product is in [-1, 1] and handle numerical instability
        let dot_product = dot_product.max(-1.0).min(1.0);
        (1.0 - dot_product).max(0.0)
    }

    fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        // Check if either vector is zero
        let a_zero = a.iter().all(|&x| x.abs() < 1e-10);
        let b_zero = b.iter().all(|&x| x.abs() < 1e-10);
        if a_zero || b_zero {
            return 0.0;
        }

        let a_normalized = Self::normalize_vector(a);
        let b_normalized = Self::normalize_vector(b);

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                unsafe {
                    let dot_product = Self::dot_product_avx2(&a_normalized, &b_normalized);
                    return dot_product.max(-1.0).min(1.0);
                }
            }
        }

        // Fallback for non-x86_64 architectures or when AVX2 is not available
        let mut dot_product = 0.0;
        for (x, y) in a_normalized.iter().zip(b_normalized.iter()) {
            dot_product += x * y;
        }
        dot_product.max(-1.0).min(1.0)
    }

    fn name(&self) -> &'static str {
        "cosine_simd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let metric = CosineDistance::new();
        
        // Test identical vectors
        let v1 = vec![1.0, 0.0, 1.0];
        assert!((metric.similarity(&v1, &v1) - 1.0).abs() < 1e-6);

        // Test orthogonal vectors
        let v2 = vec![0.0, 1.0, 0.0];
        assert!((metric.similarity(&v1, &v2) - 0.0).abs() < 1e-6);

        // Test similar vectors
        let v3 = vec![1.0, 0.5, 1.0];
        let sim = metric.similarity(&v1, &v3);
        assert!(sim > 0.9 && sim < 1.0);

        // Test empty vectors
        assert!((metric.similarity(&[], &[]) - 0.0).abs() < 1e-6);

        // Test different length vectors
        assert!((metric.similarity(&[1.0], &[1.0, 2.0]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_distance() {
        let metric = CosineDistance::new();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let c = vec![1.0, 0.0, 0.0];

        assert!((metric.calculate_distance(&a, &b) - 1.0).abs() < 1e-6);
        assert!(metric.calculate_distance(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_empty_vectors() {
        let metric = CosineDistance::new();
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];

        assert_eq!(metric.calculate_distance(&a, &b), 1.0);
    }

    #[test]
    fn test_zero_vectors() {
        let metric = CosineDistance::new();
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];

        assert_eq!(metric.calculate_distance(&a, &b), 1.0);
    }
}
