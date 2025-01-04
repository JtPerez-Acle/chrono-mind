use std::error::Error;
use chrono_mind::{
    config::Config,
    init_telemetry, shutdown_telemetry,
    memory::types::MemoryAttributes,
    storage::persistence::MemoryBackend,
    server::Server,
};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_telemetry()?;

    let config = Config::default();
    let backend = MemoryBackend::new(Default::default());
    let mut server = Server::new(config, backend);

    println!("Starting ChronoMind server...");
    server.run().await?;

    shutdown_telemetry();
    Ok(())
}
