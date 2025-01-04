use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub vector_dims: usize,
    pub quantum_enabled: bool,
    pub neural_compression: bool,
    pub temporal_fusion: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            vector_dims: 768,
            quantum_enabled: true,
            neural_compression: true,
            temporal_fusion: true,
        }
    }
}
