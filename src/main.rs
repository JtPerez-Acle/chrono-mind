use std::sync::Arc;
use vector_store::{
    storage::metrics::CosineDistance,
    memory::temporal::MemoryStorage,
    core::config::MemoryConfig,
};

#[tokio::main]
async fn main() {
    println!("Vector Store - A fast and efficient vector storage solution");
    
    // Example initialization
    let config = MemoryConfig::default();
    let metric = Arc::new(CosineDistance::new());
    let _storage = MemoryStorage::new(config, metric);
    
    // Add your code here
}
