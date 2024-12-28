use tracing_subscriber::{fmt, EnvFilter};

pub mod error;
pub mod storage;
pub mod logging;
pub mod utils;

pub use error::{Result, VectorStoreError};
pub use storage::{HnswConfig, HnswIndex, Vector, VectorStorage};

/// Initialize the vector store with logging
pub fn init() -> Result<()> {
    let log_dir = std::env::temp_dir().join("vector-store-logs");
    std::fs::create_dir_all(&log_dir)?;
    logging::init_logging(log_dir)?;
    Ok(())
}

/// Initialize the logging system with the specified log level
pub fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(env_filter)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .init();
    
    tracing::info!("Vector Store logging initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::info;

    #[test]
    fn test_init() {
        init().unwrap();
        info!("Vector store initialized successfully");
    }

    use tracing_subscriber::{fmt, EnvFilter};

    pub fn init_logging() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();
    }
}
