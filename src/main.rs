use vector_store::{
    core::config::MemoryConfig,
    memory::temporal::MemoryStorage,
    storage::metrics::CosineDistance,
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = MemoryConfig::default();
    let metric = CosineDistance::new();
    let _storage = MemoryStorage::new(metric, config)?;

    println!("Vector Store initialized successfully!");
    Ok(())
}
