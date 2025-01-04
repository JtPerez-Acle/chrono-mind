use std::time::Instant;
use tracing::info;

use crate::memory::types::MemoryStats;

/// Performance monitoring utility
pub struct PerformanceMonitor {
    name: String,
    start: Instant,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            start: Instant::now(),
        }
    }
}

impl Drop for PerformanceMonitor {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        info!(
            operation = self.name,
            duration_ms = duration.as_millis(),
            "Operation completed"
        );
    }
}

/// Monitor memory system health
pub fn monitor_memory_health(stats: &MemoryStats, capacity_warning_threshold: f32) {
    info!(
        total_memories = stats.total_memories,
        capacity_used = stats.capacity_used,
        average_importance = stats.average_importance,
        "Memory system health"
    );

    if stats.capacity_used > capacity_warning_threshold {
        info!(
            capacity_used = stats.capacity_used,
            threshold = capacity_warning_threshold,
            "Memory system capacity warning"
        );
    }

    // Log context distribution
    for (context, count) in &stats.context_distribution {
        info!(
            context = context,
            count = count,
            "Context distribution"
        );
    }
}

/// Efficiency metrics for memory system
pub struct EfficiencyMetrics {
    pub memory_utilization: f32,
    pub context_balance: f32,
    pub relationship_density: f32,
}

/// Calculate efficiency metrics for memory system
pub fn calculate_efficiency_metrics(stats: &MemoryStats) -> EfficiencyMetrics {
    // Calculate memory utilization
    let memory_utilization = stats.capacity_used / 100.0;

    // Calculate context balance (variance in context sizes)
    let avg_context_size = stats.total_memories as f32 / stats.context_distribution.len() as f32;
    let context_balance = stats.context_distribution.values()
        .map(|&count| {
            let diff = count as f32 - avg_context_size;
            diff * diff
        })
        .sum::<f32>()
        .sqrt();

    // Calculate relationship density
    let total_possible_relationships = stats.total_memories * (stats.total_memories - 1) / 2;
    let relationship_density = if total_possible_relationships > 0 {
        stats.most_connected_memories.len() as f32 / total_possible_relationships as f32
    } else {
        0.0
    };

    EfficiencyMetrics {
        memory_utilization,
        context_balance,
        relationship_density,
    }
}
