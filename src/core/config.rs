use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum number of dimensions for vectors
    pub max_dimensions: usize,
    
    /// Maximum number of memories to store
    pub max_memories: usize,
    
    /// Maximum number of relationships per memory
    pub max_relationships: usize,
    
    /// Base decay rate for memories (per hour)
    pub base_decay_rate: f32,
    
    /// Time window for memory consolidation (in hours)
    pub consolidation_window: Duration,
    
    /// Minimum importance threshold for keeping memories
    pub min_importance: f32,
    
    /// Maximum importance value
    pub max_importance: f32,
    
    /// Number of similar memories to consider for relationships
    pub similar_memory_count: usize,
    
    /// Similarity threshold for establishing relationships
    pub similarity_threshold: f32,
    
    /// Maximum size of context window
    pub max_context_window: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_dimensions: 1536, // Standard for many embedding models
            max_memories: 100_000,
            max_relationships: 50,
            base_decay_rate: 0.1,
            consolidation_window: Duration::from_secs(24 * 3600), // 24 hours
            min_importance: 0.1,
            max_importance: 1.0,
            similar_memory_count: 10,
            similarity_threshold: 0.8,
            max_context_window: 1000,
        }
    }
}

impl MemoryConfig {
    /// Create a new configuration with custom settings
    pub fn new(
        max_dimensions: usize,
        max_memories: usize,
        max_relationships: usize,
        base_decay_rate: f32,
        consolidation_window: Duration,
        min_importance: f32,
        max_importance: f32,
        similar_memory_count: usize,
        similarity_threshold: f32,
        max_context_window: usize,
    ) -> Self {
        Self {
            max_dimensions,
            max_memories,
            max_relationships,
            base_decay_rate,
            consolidation_window,
            min_importance,
            max_importance,
            similar_memory_count,
            similarity_threshold,
            max_context_window,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.max_dimensions == 0 {
            return Err("max_dimensions must be greater than 0");
        }
        if self.max_memories == 0 {
            return Err("max_memories must be greater than 0");
        }
        if self.max_relationships == 0 {
            return Err("max_relationships must be greater than 0");
        }
        if self.base_decay_rate <= 0.0 || self.base_decay_rate > 1.0 {
            return Err("base_decay_rate must be between 0 and 1");
        }
        if self.consolidation_window.as_secs() == 0 {
            return Err("consolidation_window must be greater than 0");
        }
        if self.min_importance < 0.0 || self.min_importance >= self.max_importance {
            return Err("min_importance must be between 0 and max_importance");
        }
        if self.max_importance <= self.min_importance || self.max_importance > 1.0 {
            return Err("max_importance must be between min_importance and 1");
        }
        if self.similar_memory_count == 0 {
            return Err("similar_memory_count must be greater than 0");
        }
        if self.similarity_threshold <= 0.0 || self.similarity_threshold > 1.0 {
            return Err("similarity_threshold must be between 0 and 1");
        }
        if self.max_context_window == 0 {
            return Err("max_context_window must be greater than 0");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MemoryConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_custom_config() {
        let config = MemoryConfig::new(
            768,
            50_000,
            25,
            0.05,
            Duration::from_secs(12 * 3600),
            0.2,
            0.9,
            5,
            0.7,
            500,
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let mut config = MemoryConfig::default();
        
        config.max_dimensions = 0;
        assert!(config.validate().is_err());
        
        config = MemoryConfig::default();
        config.base_decay_rate = 1.5;
        assert!(config.validate().is_err());
        
        config = MemoryConfig::default();
        config.min_importance = -0.1;
        assert!(config.validate().is_err());
        
        config = MemoryConfig::default();
        config.similarity_threshold = 0.0;
        assert!(config.validate().is_err());
    }
}
