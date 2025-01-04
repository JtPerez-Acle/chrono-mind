use std::sync::Arc;

use vector_store::{
    core::{
        config::MemoryConfig,
        error::Result,
    },
    storage::metrics::CosineDistance,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = MemoryConfig::default();
    let metric = Arc::new(CosineDistance::new());
    
    let _storage = vector_store::init_with_config(config)?;
    Ok(())
}
