use std::path::Path;
use tracing::{Level, Subscriber};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

/// Initialize logging with both console and file output
pub fn init_logging(log_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir,
        "vector-store.log",
    );

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("vector_store=debug,tower_http=debug")
        }))
        .with(fmt::Layer::new()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_writer(std::io::stdout))
        .with(fmt::Layer::new()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::CLOSE)
            .json()
            .with_writer(non_blocking));

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

/// Initialize test logging with only console output
pub fn init_test_logging() {
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("vector_store=debug")
        }))
        .with(fmt::Layer::new()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::CLOSE));

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tracing::info;

    #[test]
    fn test_file_logging() {
        let temp_dir = tempdir().unwrap();
        init_logging(temp_dir.path()).unwrap();
        
        info!(message = "Test log message", value = 42);
        
        // Log file should be created in temp_dir
        assert!(temp_dir.path().join("vector-store.log").exists());
    }

    #[test]
    fn test_console_logging() {
        init_test_logging();
        info!(message = "Test console logging", value = 42);
    }
}
