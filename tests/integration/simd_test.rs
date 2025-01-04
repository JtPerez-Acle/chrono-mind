use std::sync::Arc;
use vector_store::{
    storage::metrics::CosineDistance,
    memory::types::Vector,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[test]
fn test_simd_cosine_distance() {
    let metric = CosineDistance::new();
    
    // Test with vectors divisible by 8 for AVX2
    let v1 = Vector::new(
        "1".to_string(),
        vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    );
    let v2 = Vector::new(
        "2".to_string(),
        vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    );

    let distance = metric.calculate_distance(&v1.data, &v2.data);
    assert!((distance - 1.0).abs() < 1e-6);

    // Test with non-aligned vectors
    let v3 = Vector::new(
        "3".to_string(),
        vec![1.0, 0.0, 0.0, 0.0, 0.0],
    );
    let v4 = Vector::new(
        "4".to_string(),
        vec![0.0, 1.0, 0.0, 0.0, 0.0],
    );

    let distance = metric.calculate_distance(&v3.data, &v4.data);
    assert!((distance - 1.0).abs() < 1e-6);
}

#[test]
fn test_simd_performance() {
    let metric = CosineDistance::new();
    let size = 1024; // Large enough to see SIMD benefits

    let v1 = Vector::new(
        "1".to_string(),
        (0..size).map(|i| i as f32).collect(),
    );
    let v2 = Vector::new(
        "2".to_string(),
        (0..size).map(|i| (size - i) as f32).collect(),
    );

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        black_box(metric.calculate_distance(&v1.data, &v2.data));
    }
    let duration = start.elapsed();

    println!("SIMD Performance: {:?} for 1000 calculations of {} dimensional vectors", duration, size);
    // On modern hardware with AVX2, this should be very fast
    assert!(duration < std::time::Duration::from_millis(100));
}

#[test]
fn test_simd_edge_cases() {
    let metric = CosineDistance::new();

    // Test empty vectors
    let empty: Vec<f32> = vec![];
    assert_eq!(metric.calculate_distance(&empty, &empty), 1.0);

    // Test zero vectors
    let zero = vec![0.0; 8];
    assert_eq!(metric.calculate_distance(&zero, &zero), 1.0);

    // Test different sized vectors
    let v1 = vec![1.0, 0.0];
    let v2 = vec![1.0, 0.0, 0.0];
    assert_eq!(metric.calculate_distance(&v1, &v2), 1.0);
}
