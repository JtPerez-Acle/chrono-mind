//! Core data types: vectors, memories, and their attributes.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{Error, Result};

/// An identified embedding vector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    /// Caller-assigned unique identifier.
    pub id: String,
    /// The embedding components.
    pub data: Vec<f32>,
}

impl Vector {
    /// Create a new vector.
    pub fn new(id: impl Into<String>, data: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            data,
        }
    }
}

/// Temporal and semantic metadata attached to a [`Memory`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryAttributes {
    /// When the memory was created.
    pub timestamp: SystemTime,
    /// Importance in `[0.0, 1.0]`; decays over time via
    /// [`apply_decay`](crate::ChronoMind::apply_decay).
    pub importance: f32,
    /// Free-form grouping label (e.g. a conversation or document id).
    pub context: String,
    /// Per-hour decay rate. `0.0` means "use the store's
    /// [`base_decay_rate`](crate::Config::base_decay_rate)".
    pub decay_rate: f32,
    /// Ids of related memories.
    pub relationships: Vec<String>,
    /// Number of times this memory has been retrieved.
    pub access_count: u32,
    /// When this memory was last retrieved.
    pub last_access: SystemTime,
}

impl Default for MemoryAttributes {
    fn default() -> Self {
        let now = SystemTime::now();
        Self {
            timestamp: now,
            importance: 0.5,
            context: String::new(),
            decay_rate: 0.0,
            relationships: Vec::new(),
            access_count: 0,
            last_access: now,
        }
    }
}

/// A vector plus its temporal attributes — the unit of storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    /// The embedding vector.
    pub vector: Vector,
    /// Temporal and semantic metadata.
    pub attributes: MemoryAttributes,
}

impl Memory {
    /// Create a memory with the given vector and attributes.
    pub fn new(vector: Vector, attributes: MemoryAttributes) -> Self {
        Self { vector, attributes }
    }

    /// Create a memory with default attributes.
    pub fn from_vector(vector: Vector) -> Self {
        Self::new(vector, MemoryAttributes::default())
    }

    /// Age of the memory relative to `now`.
    pub fn age(&self, now: SystemTime) -> Duration {
        now.duration_since(self.attributes.timestamp)
            .unwrap_or(Duration::ZERO)
    }

    /// Validate this memory against a store configuration.
    pub fn validate(&self, config: &Config) -> Result<()> {
        if self.vector.id.is_empty() {
            return Err(Error::InvalidVector("id must not be empty".into()));
        }
        if self.vector.data.len() != config.dimensions {
            return Err(Error::InvalidDimensions {
                got: self.vector.data.len(),
                expected: config.dimensions,
            });
        }
        if self.vector.data.iter().any(|x| !x.is_finite()) {
            return Err(Error::InvalidVector(format!(
                "vector {} contains NaN or infinite components",
                self.vector.id
            )));
        }
        if !self.attributes.importance.is_finite()
            || !(0.0..=1.0).contains(&self.attributes.importance)
        {
            return Err(Error::InvalidImportance(self.attributes.importance));
        }
        if !self.attributes.decay_rate.is_finite() || self.attributes.decay_rate < 0.0 {
            return Err(Error::InvalidVector(format!(
                "vector {} has an invalid decay rate",
                self.vector.id
            )));
        }
        Ok(())
    }
}

/// Aggregate statistics for a store, as returned by
/// [`stats`](crate::ChronoMind::stats).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Number of memories currently stored.
    pub total_memories: usize,
    /// Total number of f32 components across all stored vectors.
    pub total_components: usize,
    /// Fraction of `max_memories` in use, in `[0.0, 1.0]`.
    pub capacity_used: f64,
    /// Mean importance across all memories (`0.0` when empty).
    pub average_importance: f32,
    /// Number of memories per context label.
    pub context_distribution: HashMap<String, usize>,
    /// Ids most referenced by other memories' relationships, descending.
    pub most_referenced: Vec<(String, usize)>,
}

/// Summary of the memories sharing a context label, as returned by
/// [`context_summary`](crate::ChronoMind::context_summary).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContextSummary {
    /// The context label.
    pub context: String,
    /// Number of memories in the context.
    pub memory_count: usize,
    /// Mean importance of the context's memories.
    pub average_importance: f32,
    /// Component-wise mean of the context's vectors.
    pub centroid: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            dimensions: 4,
            ..Config::default()
        }
    }

    fn memory(data: Vec<f32>) -> Memory {
        Memory::from_vector(Vector::new("m1", data))
    }

    #[test]
    fn valid_memory_passes() {
        assert!(memory(vec![0.1, 0.2, 0.3, 0.4]).validate(&config()).is_ok());
    }

    #[test]
    fn wrong_dimensions_rejected() {
        let err = memory(vec![0.1, 0.2]).validate(&config()).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidDimensions {
                got: 2,
                expected: 4
            }
        ));
    }

    #[test]
    fn nan_component_rejected() {
        let err = memory(vec![0.1, f32::NAN, 0.3, 0.4])
            .validate(&config())
            .unwrap_err();
        assert!(matches!(err, Error::InvalidVector(_)));
    }

    #[test]
    fn empty_id_rejected() {
        let mut m = memory(vec![0.1, 0.2, 0.3, 0.4]);
        m.vector.id.clear();
        assert!(matches!(
            m.validate(&config()),
            Err(Error::InvalidVector(_))
        ));
    }

    #[test]
    fn out_of_range_importance_rejected() {
        let mut m = memory(vec![0.1, 0.2, 0.3, 0.4]);
        m.attributes.importance = 1.5;
        assert!(matches!(
            m.validate(&config()),
            Err(Error::InvalidImportance(_))
        ));
    }
}
