use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use opentelemetry::{
    metrics::{Counter, Histogram, Meter, MeterProvider, Unit},
    KeyValue,
};
use opentelemetry_sdk::metrics::MeterProvider as SdkMeterProvider;
use parking_lot::RwLock;
use tracing::{debug, warn};

use crate::memory::types::MemoryStats;

#[derive(Clone, Debug)]
pub struct MetricsRegistry {
    meter: Meter,
    operation_duration: Histogram<f64>,
    memory_usage: Counter<u64>,
    vector_ops: Counter<u64>,
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        let provider = SdkMeterProvider::builder().build();
        let meter = provider.meter("vector_store");
        
        let operation_duration = meter
            .f64_histogram("operation_duration")
            .with_description("Duration of operations in milliseconds")
            .with_unit(Unit::new("ms"))
            .init();

        let memory_usage = meter
            .u64_counter("memory_usage")
            .with_description("Memory usage in bytes")
            .with_unit(Unit::new("bytes"))
            .init();

        let vector_ops = meter
            .u64_counter("vector_operations")
            .with_description("Number of vector operations")
            .init();

        Self {
            meter,
            operation_duration,
            memory_usage,
            vector_ops,
        }
    }
}

impl MetricsRegistry {
    pub fn record_operation_duration(&self, operation: &str, duration: Duration) {
        let attributes = &[KeyValue::new("operation", operation.to_string())];
        self.operation_duration.record(duration.as_secs_f64() * 1000.0, attributes);
        debug!("Operation {} took {:?}", operation, duration);
    }

    pub fn record_memory_usage(&self, bytes: u64, context: &str) {
        let attributes = &[KeyValue::new("context", context.to_string())];
        self.memory_usage.add(bytes, attributes);
        debug!("Memory usage for {}: {} bytes", context, bytes);
    }

    pub fn record_vector_operation(&self, operation_type: &str) {
        let attributes = &[KeyValue::new("type", operation_type.to_string())];
        self.vector_ops.add(1, attributes);
        debug!("Vector operation recorded: {}", operation_type);
    }
}

#[derive(Debug)]
pub struct PerformanceMonitor {
    name: String,
    start: Instant,
    metrics: Arc<MetricsRegistry>,
}

impl PerformanceMonitor {
    pub fn new(name: &str, metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            name: name.to_string(),
            start: Instant::now(),
            metrics,
        }
    }

    pub fn record_metric(&self, value: f64, attributes: &[KeyValue]) {
        self.metrics.operation_duration.record(value, attributes);
        debug!("Metric recorded for {}: {}", self.name, value);
    }
}

impl Drop for PerformanceMonitor {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.metrics.record_operation_duration(&self.name, duration);
    }
}

#[derive(Debug)]
pub struct MemoryMonitor {
    metrics: Arc<MetricsRegistry>,
    stats: Arc<RwLock<MemoryStats>>,
    leak_threshold: usize,
}

impl MemoryMonitor {
    pub fn new(metrics: Arc<MetricsRegistry>, stats: Arc<RwLock<MemoryStats>>, leak_threshold: usize) -> Self {
        Self {
            metrics,
            stats,
            leak_threshold,
        }
    }

    pub fn check_memory_usage(&self, used_bytes: u64, context: &str) {
        self.metrics.record_memory_usage(used_bytes, context);

        if used_bytes > self.leak_threshold as u64 {
            warn!(
                "Memory usage exceeds threshold in {}: {} bytes (threshold: {})",
                context, used_bytes, self.leak_threshold
            );
        }
    }

    pub fn monitor_health(&self) {
        let stats = self.stats.read();
        let attributes = &[
            KeyValue::new("total_memories", stats.total_memories as i64),
            KeyValue::new("capacity_used", stats.capacity_used as f64),
        ];

        self.metrics.memory_usage.add(stats.capacity_used as u64, attributes);

        if stats.capacity_used > (self.leak_threshold as f64) {
            warn!(
                capacity_used = stats.capacity_used,
                threshold = self.leak_threshold,
                "Potential memory leak detected"
            );
        }
    }
}

pub fn calculate_efficiency_metrics(total_ops: u64, duration: Duration, memory_used: u64) -> f64 {
    let ops_per_second = total_ops as f64 / duration.as_secs_f64();
    let memory_efficiency = ops_per_second / memory_used as f64;
    memory_efficiency
}
