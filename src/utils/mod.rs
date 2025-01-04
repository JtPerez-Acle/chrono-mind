pub mod monitoring;
pub mod validation;

pub use validation::{
    validate_vector_dimensions,
    validate_vector_data,
    validate_temporal_vector,
};
pub use monitoring::{PerformanceMonitor, MetricsRegistry, MemoryMonitor, calculate_efficiency_metrics};
