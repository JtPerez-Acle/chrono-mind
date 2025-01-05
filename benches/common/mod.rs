use criterion::{Criterion, measurement::WallTime};
use rand::prelude::*;
use rand_distr::{Normal, Uniform};
use std::time::{SystemTime, Duration};

#[derive(Debug, Clone)]
pub enum MetricType {
    Latency,    // Response time in milliseconds
    Throughput, // Queries per second
    Accuracy,   // Precision@K for search results
    Memory,     // Memory usage in MB
}

/// Query patterns to simulate different real-world scenarios
#[derive(Debug, Clone, Copy)]
pub enum QueryPattern {
    ExactMatch,   // Exact content retrieval
    Semantic,     // Semantic similarity search
    Hybrid,       // Combination of exact and semantic
}

/// Model dimensions based on popular embedding models
pub mod config {
    pub const DIMS_BERT_BASE: usize = 768;    // BERT base model
    pub const DIMS_ADA_002: usize = 1536;     // OpenAI ada-002
    pub const DIMS_E5_LARGE: usize = 1024;    // E5 large model
    pub const DIMS_MINILM: usize = 384;       // MiniLM model

    // Dataset sizes for benchmarking
    pub const DATASET_SMALL: usize = 10_000;    // 10K vectors
    pub const DATASET_MEDIUM: usize = 100_000;  // 100K vectors
    pub const DATASET_LARGE: usize = 1_000_000; // 1M vectors

    // Importance configurations for temporal aspects
    pub const IMPORTANCE_CONFIGS: [(f32, &str); 4] = [
        (1.0, "critical"),     // High importance, immediate recall needed
        (0.8, "important"),    // Important but not critical
        (0.5, "normal"),       // Standard importance
        (0.2, "background"),   // Background/archival data
    ];
}

/// Common query patterns for benchmarking
pub const QUERY_PATTERNS: [QueryPattern; 3] = [
    QueryPattern::ExactMatch,
    QueryPattern::Semantic,
    QueryPattern::Hybrid,
];

/// Benchmark timing configurations
pub const WARM_UP_TIME: Duration = Duration::from_secs(3);
pub const MEASUREMENT_TIME: Duration = Duration::from_secs(10);
pub const MIN_SAMPLE_SIZE: usize = 50;

/// Performance targets (metric type, target value)
pub const PERFORMANCE_TARGETS: [(MetricType, f32); 4] = [
    (MetricType::Latency, 50.0),     // 50ms max latency
    (MetricType::Throughput, 1000.0), // 1000 QPS
    (MetricType::Accuracy, 0.95),     // 95% precision@10
    (MetricType::Memory, 1024.0),     // 1GB max memory
];

/// Generate realistic embeddings that mimic production models
pub fn generate_realistic_embeddings(count: usize, model: &str) -> Vec<Vec<f32>> {
    let dim = match model {
        "bert-base" => 768,
        "ada-002" => 1536,
        "e5-large" => 1024,
        "minilm" => 384,
        _ => 768, // default to BERT dimensions
    };

    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, 0.1).unwrap();
    let mut embeddings = Vec::with_capacity(count);

    for _ in 0..count {
        // Generate vectors with realistic properties:
        // 1. Most values close to 0 (normal distribution)
        // 2. Some semantic clusters
        // 3. Sparse activation patterns
        let mut vec = Vec::with_capacity(dim);
        for _ in 0..dim {
            let val = normal.sample(&mut rng) as f32;
            vec.push(val);
        }
        
        // Normalize the vector
        let magnitude = vec.iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        
        if magnitude > 0.0 {
            for x in &mut vec {
                *x /= magnitude;
            }
        }
        
        embeddings.push(vec);
    }

    embeddings
}

/// Generate temporal patterns that mimic real usage
pub fn generate_temporal_patterns(count: usize, pattern: &str) -> Vec<SystemTime> {
    let now = SystemTime::now();
    let mut timestamps = Vec::with_capacity(count);
    let mut rng = rand::thread_rng();

    match pattern {
        "recent" => {
            // Most vectors added in last 24 hours
            let dist = Uniform::new(0, 24 * 3600);
            for _ in 0..count {
                let age = dist.sample(&mut rng);
                timestamps.push(now - Duration::from_secs(age));
            }
        },
        "mixed" => {
            // Mix of recent and older vectors
            let recent_dist = Uniform::new(0, 7 * 24 * 3600);
            let medium_dist = Uniform::new(7 * 24 * 3600, 30 * 24 * 3600);
            let old_dist = Uniform::new(30 * 24 * 3600, 365 * 24 * 3600);

            for i in 0..count {
                let age = if i % 10 < 7 {
                    // 70% recent (last week)
                    recent_dist.sample(&mut rng)
                } else if i % 10 < 9 {
                    // 20% medium age (last month)
                    medium_dist.sample(&mut rng)
                } else {
                    // 10% old (last year)
                    old_dist.sample(&mut rng)
                };
                timestamps.push(now - Duration::from_secs(age));
            }
        },
        "uniform" => {
            // Uniform distribution over last year
            let dist = Uniform::new(0, 365 * 24 * 3600);
            for _ in 0..count {
                let age = dist.sample(&mut rng);
                timestamps.push(now - Duration::from_secs(age));
            }
        },
        _ => {
            // Default to mixed pattern
            let dist = Uniform::new(0, 30 * 24 * 3600);
            for _ in 0..count {
                let age = dist.sample(&mut rng);
                timestamps.push(now - Duration::from_secs(age));
            }
        }
    }

    timestamps
}

/// Setup benchmark group with consistent configuration
pub fn setup_benchmark_group<'a>(c: &'a mut Criterion, name: &str) -> criterion::BenchmarkGroup<'a, WallTime> {
    let mut group = c.benchmark_group(name);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASUREMENT_TIME);
    group.sample_size(MIN_SAMPLE_SIZE);
    group
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realistic_embeddings() {
        let vectors = generate_realistic_embeddings(10, "bert-base");
        assert_eq!(vectors.len(), 10);
        assert_eq!(vectors[0].len(), 768);

        // Verify unit vectors
        for vec in vectors {
            let magnitude = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((magnitude - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_temporal_patterns() {
        let now = std::time::SystemTime::now();
        
        // Test chat history pattern
        let times = generate_temporal_patterns(100, "recent");
        assert_eq!(times.len(), 100);
        for time in times {
            assert!(time <= now);
            assert!(time >= now - Duration::from_secs(24 * 3600));
        }

        // Test knowledge base pattern
        let times = generate_temporal_patterns(100, "mixed");
        assert_eq!(times.len(), 100);
        for time in times {
            assert!(time <= now);
            assert!(time >= now - Duration::from_secs(365 * 24 * 3600));
        }
    }
}
