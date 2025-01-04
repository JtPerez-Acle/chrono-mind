use std::f32;

/// Trait for implementing distance/similarity metrics
pub trait DistanceMetric: Send + Sync {
    /// Calculate similarity between two vectors
    fn similarity(&self, a: &[f32], b: &[f32]) -> f32;
}

/// Cosine similarity implementation
#[derive(Clone)]
pub struct CosineDistance;

impl CosineDistance {
    /// Create a new CosineDistance instance
    pub fn new() -> Self {
        Self
    }
}

impl DistanceMetric for CosineDistance {
    fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    const EPSILON: f32 = 1e-6;

    #[test]
    fn test_cosine_similarity() {
        let metric = CosineDistance::new();
        
        // Test identical vectors
        let v1 = vec![1.0, 0.0, 1.0];
        assert!((metric.similarity(&v1, &v1) - 1.0).abs() < EPSILON);

        // Test orthogonal vectors
        let v2 = vec![0.0, 1.0, 0.0];
        assert!((metric.similarity(&v1, &v2) - 0.0).abs() < EPSILON);

        // Test similar vectors
        let v3 = vec![1.0, 0.5, 1.0];
        let sim = metric.similarity(&v1, &v3);
        assert!(sim > 0.9 && sim < 1.0);

        // Test empty vectors
        assert!((metric.similarity(&[], &[]) - 0.0).abs() < EPSILON);

        // Test different length vectors
        assert!((metric.similarity(&[1.0], &[1.0, 2.0]) - 0.0).abs() < EPSILON);
    }
}
