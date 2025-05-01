use std::sync::Arc;
use std::time::Duration;
use opentelemetry::KeyValue;
use vector_store::{
    memory::types::MemoryStats,
    utils::monitoring::{MetricsRegistry, PerformanceMonitor, MemoryMonitor, calculate_efficiency_metrics},
};
use parking_lot::RwLock;
use std::collections::HashMap;

#[tokio::test]
async fn test_metrics_registry() {
    // Create a metrics registry
    let registry = MetricsRegistry::default();

    // Test recording operation duration
    let duration = Duration::from_millis(100);
    registry.record_operation_duration("test_operation", duration);

    // Test recording memory usage
    registry.record_memory_usage(1024, "test_context");

    // Test recording vector operation
    registry.record_vector_operation("insert");

    // No assertions needed as we're just testing that the methods don't panic
}

#[tokio::test]
async fn test_performance_monitor() {
    // Create a metrics registry
    let registry = Arc::new(MetricsRegistry::default());

    // Create a performance monitor
    let monitor = PerformanceMonitor::new("test_operation", registry.clone());

    // Record a metric
    let attributes = &[KeyValue::new("test", "value")];
    monitor.record_metric(42.0, attributes);

    // Sleep to simulate work
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Monitor will be dropped here, which should record the duration
}

#[tokio::test]
async fn test_memory_monitor() {
    // Create a metrics registry
    let registry = Arc::new(MetricsRegistry::default());

    // Create memory stats
    let stats = MemoryStats {
        total_memories: 100,
        total_size: 1024,
        avg_vector_size: 10.24,
        capacity_used: 1024.0,
        average_importance: 0.5,
        context_distribution: HashMap::new(),
        most_connected_memories: Vec::new(),
    };

    let stats_arc = Arc::new(RwLock::new(stats));

    // Create a memory monitor with a low threshold to trigger warnings
    let monitor = MemoryMonitor::new(registry.clone(), stats_arc.clone(), 512);

    // Check memory usage (should trigger warning)
    monitor.check_memory_usage(1024, "test_context");

    // Monitor health (should trigger warning)
    monitor.monitor_health();

    // No assertions needed as we're just testing that the methods don't panic
}

#[tokio::test]
async fn test_calculate_efficiency_metrics() {
    // Test with some sample values
    let total_ops = 1000;
    let duration = Duration::from_secs(2);
    let memory_used = 1024;

    let efficiency = calculate_efficiency_metrics(total_ops, duration, memory_used);

    // Calculate expected value: (1000 / 2) / 1024 = 0.48828125
    let expected = 500.0 / 1024.0;

    assert!((efficiency - expected).abs() < f64::EPSILON);
}
