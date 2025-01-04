use crate::{
    core::{
        config::MemoryConfig,
        error::{MemoryError, Result},
    },
    memory::types::{Vector, TemporalVector},
};

/// Validate vector dimensions against config
pub fn validate_vector_dimensions(vector: &Vector, config: &MemoryConfig) -> Result<()> {
    if vector.data.len() != config.max_dimensions {
        return Err(MemoryError::InvalidDimensions {
            got: vector.data.len(),
            expected: config.max_dimensions,
        });
    }
    Ok(())
}

/// Validate vector data
pub fn validate_vector_data(vector: &Vector) -> Result<()> {
    if vector.data.is_empty() {
        return Err(MemoryError::InvalidVectorData(
            "Vector data cannot be empty".to_string(),
        ));
    }

    for value in &vector.data {
        if !value.is_finite() {
            return Err(MemoryError::InvalidVectorData(
                "Vector data contains non-finite values".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validate temporal vector attributes
pub fn validate_temporal_vector(memory: &TemporalVector) -> Result<()> {
    if memory.attributes.importance < 0.0 || memory.attributes.importance > 1.0 {
        return Err(MemoryError::InvalidImportance(memory.attributes.importance));
    }

    if memory.attributes.decay_rate < 0.0 || memory.attributes.decay_rate > 1.0 {
        return Err(MemoryError::InvalidAttributes(format!(
            "Decay rate must be between 0 and 1, got {}",
            memory.attributes.decay_rate
        )));
    }

    if memory.attributes.context.is_empty() {
        return Err(MemoryError::InvalidAttributes(
            "Context cannot be empty".to_string(),
        ));
    }

    Ok(())
}
