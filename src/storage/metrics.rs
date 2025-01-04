use std::f32;
use std::sync::Arc;
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

        let mut result = 0.0;
        let mut temp = [0.0f32; 8];
        _mm256_storeu_ps(temp.as_mut_ptr(), sum);
        
        for i in 0..8 {
            result += temp[i];
        }

        // Handle remaining elements
        for i in n..a.len() {
            result += a[i] * b[i];
        }

        result
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn vector_magnitude_avx2(v: &[f32]) -> f32 {
        let mut sum = _mm256_setzero_ps();
        let n = v.len() / 8 * 8;

        for i in (0..n).step_by(8) {
            let va = _mm256_loadu_ps(&v[i]);
            sum = _mm256_fmadd_ps(va, va, sum);
        }

        let mut result = 0.0;
        let mut temp = [0.0f32; 8];
        _mm256_storeu_ps(temp.as_mut_ptr(), sum);
        
        for i in 0..8 {
            result += temp[i];
        }

        // Handle remaining elements
        for i in n..v.len() {
            result += v[i] * v[i];
        }

        result.sqrt()
    }
}

impl DistanceMetric for CosineDistance {
    fn calculate_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 1.0;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                unsafe {
                    let dot_product = Self::dot_product_avx2(a, b);
                    let norm_a = Self::vector_magnitude_avx2(a);
                    let norm_b = Self::vector_magnitude_avx2(b);

                    if norm_a == 0.0 || norm_b == 0.0 {
                        return 1.0;
                    }

                    return 1.0 - (dot_product / (norm_a * norm_b));
                }
            }
        }

        // Fallback for non-x86_64 architectures or when AVX2 is not available
        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for (x, y) in a.iter().zip(b.iter()) {
            dot_product += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 1.0;
        }

        1.0 - (dot_product / (norm_a.sqrt() * norm_b.sqrt()))
    }

    fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                unsafe {
                    let dot_product = Self::dot_product_avx2(a, b);
                    let norm_a = Self::vector_magnitude_avx2(a);
                    let norm_b = Self::vector_magnitude_avx2(b);

                    if norm_a == 0.0 || norm_b == 0.0 {
                        return 0.0;
                    }

                    return (dot_product / (norm_a * norm_b)).min(1.0);
                }
            }
        }

        // Fallback for non-x86_64 architectures or when AVX2 is not available
        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for (x, y) in a.iter().zip(b.iter()) {
            dot_product += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot_product / (norm_a.sqrt() * norm_b.sqrt())).min(1.0)
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
