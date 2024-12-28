use std::f32;
use std::fmt::Debug;

/// Trait for vector distance metrics
pub trait DistanceMetric: Send + Sync + Debug {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy)]
pub struct EuclideanDistance;

impl DistanceMetric for EuclideanDistance {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt()
    }

    fn name(&self) -> &'static str {
        "euclidean"
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CosineDistance;

impl DistanceMetric for CosineDistance {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return 1.0; // Maximum distance for zero vectors
        }
        
        1.0 - (dot_product / (norm_a * norm_b))
    }

    fn name(&self) -> &'static str {
        "cosine"
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DotProductDistance;

impl DistanceMetric for DotProductDistance {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
        -(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>())
    }

    fn name(&self) -> &'static str {
        "dot_product"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn test_euclidean_distance() {
        let metric = EuclideanDistance;
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let distance = metric.distance(&a, &b);
        assert!((distance - f32::sqrt(2.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_distance() {
        let metric = CosineDistance;
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let distance = metric.distance(&a, &b);
        assert!((distance - 1.0).abs() < 1e-6); // Orthogonal vectors should have distance 1
    }

    #[test]
    fn test_dot_product_distance() {
        let metric = DotProductDistance;
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = metric.distance(&a, &b);
        assert_eq!(distance, -(32.0)); // -(1*4 + 2*5 + 3*6)
    }

    #[test]
    fn test_zero_vector_cosine() {
        let metric = CosineDistance;
        let zero = vec![0.0, 0.0];
        let a = vec![1.0, 0.0];
        assert_eq!(metric.distance(&zero, &a), 1.0);
        assert_eq!(metric.distance(&a, &zero), 1.0);
    }
}
