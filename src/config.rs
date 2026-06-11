//! Store and index configuration.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Parameters for the HNSW index.
///
/// These follow the standard HNSW nomenclature from Malkov & Yashunin (2018).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexParams {
    /// Number of bidirectional links created per node and layer (`M`).
    ///
    /// Layer 0 allows `2 * max_connections` links.
    pub max_connections: usize,

    /// Size of the dynamic candidate list during construction (`efConstruction`).
    pub ef_construction: usize,

    /// Size of the dynamic candidate list during search (`efSearch`).
    ///
    /// Searches always explore at least this many candidates before temporal
    /// reranking; raising it trades latency for recall.
    pub ef_search: usize,
}

impl Default for IndexParams {
    fn default() -> Self {
        Self {
            max_connections: 16,
            ef_construction: 200,
            ef_search: 50,
        }
    }
}

/// Configuration for a [`ChronoMind`](crate::ChronoMind) store.
///
/// Construct with [`Config::default`] and adjust fields, or use
/// [`Config::builder`] for a fluent interface. All configurations are
/// validated when the store is created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Dimensionality every stored vector must have.
    pub dimensions: usize,

    /// Maximum number of memories the store will hold.
    pub max_memories: usize,

    /// Default decay rate, per hour, applied to memories whose
    /// [`decay_rate`](crate::MemoryAttributes::decay_rate) is zero.
    ///
    /// Temporal relevance of a memory of age `t` hours is `exp(-rate * t)`.
    pub base_decay_rate: f32,

    /// Weight of temporal relevance in search scoring, in `[0.0, 1.0]`.
    ///
    /// `0.0` ranks purely by vector distance; `1.0` purely by recency.
    /// See [`ChronoMind::search`](crate::ChronoMind::search) for the exact formula.
    pub temporal_weight: f32,

    /// Cosine similarity above which two memories are considered duplicates
    /// by [`consolidate`](crate::ChronoMind::consolidate), in `(0.0, 1.0)`.
    pub similarity_threshold: f32,

    /// Maximum number of relationship links kept per memory.
    pub max_relationships: usize,

    /// HNSW index parameters.
    pub index: IndexParams,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dimensions: 768, // BERT-base embedding size
            max_memories: 100_000,
            base_decay_rate: 0.1,
            temporal_weight: 0.3,
            similarity_threshold: 0.95,
            max_relationships: 50,
            index: IndexParams::default(),
        }
    }
}

impl Config {
    /// Start building a configuration from the defaults.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder {
            config: Self::default(),
        }
    }

    /// Validate the configuration, returning a descriptive error for the
    /// first violated constraint.
    pub fn validate(&self) -> Result<()> {
        if self.dimensions == 0 {
            return Err(Error::Config("dimensions must be greater than 0".into()));
        }
        if self.max_memories == 0 {
            return Err(Error::Config("max_memories must be greater than 0".into()));
        }
        if !self.base_decay_rate.is_finite() || self.base_decay_rate <= 0.0 {
            return Err(Error::Config(
                "base_decay_rate must be a positive finite number".into(),
            ));
        }
        if !self.temporal_weight.is_finite() || !(0.0..=1.0).contains(&self.temporal_weight) {
            return Err(Error::Config(
                "temporal_weight must be within [0.0, 1.0]".into(),
            ));
        }
        if !self.similarity_threshold.is_finite()
            || self.similarity_threshold <= 0.0
            || self.similarity_threshold >= 1.0
        {
            return Err(Error::Config(
                "similarity_threshold must be within (0.0, 1.0)".into(),
            ));
        }
        if self.max_relationships == 0 {
            return Err(Error::Config(
                "max_relationships must be greater than 0".into(),
            ));
        }
        if self.index.max_connections < 2 {
            return Err(Error::Config(
                "index.max_connections must be at least 2".into(),
            ));
        }
        if self.index.ef_construction < self.index.max_connections {
            return Err(Error::Config(
                "index.ef_construction must be at least index.max_connections".into(),
            ));
        }
        if self.index.ef_search == 0 {
            return Err(Error::Config(
                "index.ef_search must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

/// Fluent builder for [`Config`].
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    /// Set the required vector dimensionality.
    pub fn dimensions(mut self, dimensions: usize) -> Self {
        self.config.dimensions = dimensions;
        self
    }

    /// Set the maximum number of memories.
    pub fn max_memories(mut self, max_memories: usize) -> Self {
        self.config.max_memories = max_memories;
        self
    }

    /// Set the default per-hour decay rate.
    pub fn base_decay_rate(mut self, rate: f32) -> Self {
        self.config.base_decay_rate = rate;
        self
    }

    /// Set the temporal weight used in search scoring.
    pub fn temporal_weight(mut self, weight: f32) -> Self {
        self.config.temporal_weight = weight;
        self
    }

    /// Set the similarity threshold for consolidation.
    pub fn similarity_threshold(mut self, threshold: f32) -> Self {
        self.config.similarity_threshold = threshold;
        self
    }

    /// Set the maximum relationships per memory.
    pub fn max_relationships(mut self, max: usize) -> Self {
        self.config.max_relationships = max;
        self
    }

    /// Set the HNSW index parameters.
    pub fn index(mut self, index: IndexParams) -> Self {
        self.config.index = index;
        self
    }

    /// Validate and produce the configuration.
    pub fn build(self) -> Result<Config> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn builder_produces_valid_config() {
        let config = Config::builder()
            .dimensions(128)
            .max_memories(10_000)
            .temporal_weight(0.5)
            .build()
            .unwrap();
        assert_eq!(config.dimensions, 128);
        assert_eq!(config.max_memories, 10_000);
    }

    type Mutation = Box<dyn Fn(&mut Config)>;

    #[test]
    fn invalid_configs_are_rejected() {
        let cases: Vec<Mutation> = vec![
            Box::new(|c| c.dimensions = 0),
            Box::new(|c| c.max_memories = 0),
            Box::new(|c| c.base_decay_rate = 0.0),
            Box::new(|c| c.base_decay_rate = f32::NAN),
            Box::new(|c| c.temporal_weight = -0.1),
            Box::new(|c| c.temporal_weight = 1.5),
            Box::new(|c| c.similarity_threshold = 0.0),
            Box::new(|c| c.similarity_threshold = 1.0),
            Box::new(|c| c.max_relationships = 0),
            Box::new(|c| c.index.max_connections = 1),
            Box::new(|c| c.index.ef_construction = 1),
            Box::new(|c| c.index.ef_search = 0),
        ];
        for (i, mutate) in cases.iter().enumerate() {
            let mut config = Config::default();
            mutate(&mut config);
            assert!(config.validate().is_err(), "case {i} should be rejected");
        }
    }
}
