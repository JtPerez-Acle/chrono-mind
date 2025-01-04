use tracing::{info, Level};
use tracing_subscriber::{fmt, EnvFilter};
use std::sync::Once;

/// Initialize the logging system with default settings
static INIT: Once = Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        let filter = EnvFilter::from_default_env()
            .add_directive(Level::INFO.into())
            .add_directive("vector_store=debug".parse().unwrap());

        fmt()
            .with_env_filter(filter)
            .with_thread_ids(true)
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .init();

        info!("Logging system initialized");
    });
}

/// Initialize logging with custom filter
pub fn init_logging_with_filter(filter: EnvFilter) {
    INIT.call_once(|| {
        fmt()
            .with_env_filter(filter)
            .with_thread_ids(true)
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .init();

        info!("Logging system initialized with custom filter");
    });
}
