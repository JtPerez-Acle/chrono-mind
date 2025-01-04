use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum number of memories to store
    pub max_memories: usize,
    
    /// Maximum dimension size for vectors
    pub max_dimensions: usize,
    
    /// Threshold for memory consolidation similarity
    pub consolidation_threshold: f32,
    
    /// Default decay rate for new memories
    pub default_decay_rate: f32,
    
    /// Time window for automatic consolidation
    pub consolidation_window: Duration,
    
    /// Minimum importance score to keep a memory
    pub min_importance_threshold: f32,
    
    /// Maximum relationships per memory
    pub max_relationships: usize,
    
    /// Whether to enable automatic memory cleanup
    pub enable_auto_cleanup: bool,
    
    /// Interval for automatic memory cleanup
    pub cleanup_interval: Duration,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memories: 1_000_000,
            max_dimensions: 1024,
            consolidation_threshold: 0.85,
            default_decay_rate: 0.1,
            consolidation_window: Duration::from_secs(3600), // 1 hour
            min_importance_threshold: 0.1,
            max_relationships: 100,
            enable_auto_cleanup: true,
            cleanup_interval: Duration::from_secs(86400), // 24 hours
        }
    }
}

impl MemoryConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_memories == 0 {
            return Err("max_memories must be greater than 0".to_string());
        }
        if self.max_dimensions == 0 {
            return Err("max_dimensions must be greater than 0".to_string());
        }
        if !(0.0..=1.0).contains(&self.consolidation_threshold) {
            return Err("consolidation_threshold must be between 0 and 1".to_string());
        }
        if !(0.0..=1.0).contains(&self.default_decay_rate) {
            return Err("default_decay_rate must be between 0 and 1".to_string());
        }
        if !(0.0..=1.0).contains(&self.min_importance_threshold) {
            return Err("min_importance_threshold must be between 0 and 1".to_string());
        }
        if self.max_relationships == 0 {
            return Err("max_relationships must be greater than 0".to_string());
        }
        Ok(())
    }
}
