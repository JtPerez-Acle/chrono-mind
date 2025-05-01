use vector_store::core::logging::{init_logging, init_logging_with_filter};
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn test_init_logging() {
    // Test basic initialization
    init_logging();
    
    // Calling it again should be a no-op due to the Once guard
    init_logging();
    
    // This test mainly verifies that the function doesn't panic
    // We can't easily verify the actual logging output in a unit test
}

#[tokio::test]
async fn test_init_logging_with_filter() {
    // Create a custom filter
    let filter = EnvFilter::from_default_env()
        .add_directive("vector_store=trace".parse().unwrap())
        .add_directive("test=debug".parse().unwrap());
    
    // Initialize with custom filter
    init_logging_with_filter(filter);
    
    // Calling it again should be a no-op due to the Once guard
    init_logging();
    
    // This test mainly verifies that the function doesn't panic
    // We can't easily verify the actual logging output in a unit test
}
