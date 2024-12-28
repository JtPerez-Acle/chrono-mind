use std::time::Instant;
use tracing::info;

/// A simple timer utility for performance measurements
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            name: name.into(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        info!(
            operation = self.name.as_str(),
            duration_ms = duration.as_millis(),
            "Operation completed"
        );
    }
}
