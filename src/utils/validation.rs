use crate::{
    core::error::MemoryError,
    memory::types::{Vector, TemporalVector},
    core::config::MemoryConfig,
};

/// Validate a vector's dimensions against configuration
pub fn validate_dimensions(vector: &Vector, config: &MemoryConfig) -> Result<(), MemoryError> {
    if vector.data.len() > config.max_dimensions {
        return Err(MemoryError::DimensionMismatch {
            expected: config.max_dimensions,
            actual: vector.data.len(),
        });
    }
    Ok(())
}

/// Validate temporal vector attributes
pub fn validate_temporal_vector(memory: &TemporalVector) -> Result<(), MemoryError> {
    if memory.attributes.importance < 0.0 || memory.attributes.importance > 1.0 {
        return Err(MemoryError::InvalidAttributes(
            "Importance must be between 0 and 1".to_string(),
        ));
    }

    if memory.attributes.decay_rate < 0.0 || memory.attributes.decay_rate > 1.0 {
        return Err(MemoryError::InvalidAttributes(
            "Decay rate must be between 0 and 1".to_string(),
        ));
    }

    if memory.attributes.context.is_empty() {
        return Err(MemoryError::InvalidAttributes(
            "Context cannot be empty".to_string(),
        ));
    }

    Ok(())
}

/// Validate relationships between memories
pub fn validate_relationships(
    memory: &TemporalVector,
    config: &MemoryConfig,
) -> Result<(), MemoryError> {
    if memory.attributes.relationships.len() > config.max_relationships {
        return Err(MemoryError::InvalidAttributes(format!(
            "Number of relationships ({}) exceeds maximum allowed ({})",
            memory.attributes.relationships.len(),
            config.max_relationships
        )));
    }

    Ok(())
}
