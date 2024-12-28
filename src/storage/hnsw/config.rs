use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Maximum number of connections per node
    pub m: usize,
    /// Size of dynamic candidate list during construction
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search
    pub ef: usize,
    /// Maximum number of layers
    pub max_layers: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef: 10,
            max_layers: 4,
        }
    }
}

impl Config {
    pub fn new(m: usize, ef_construction: usize, ef: usize, max_layers: usize) -> Self {
        Self {
            m,
            ef_construction,
            ef,
            max_layers,
        }
    }
}
