//! ChronoMind — a temporal vector store for AI agent memory.
//!
//! ChronoMind stores embedding vectors together with temporal attributes
//! (creation time, importance, decay rate, access history) and ranks search
//! results by a documented combination of geometric distance and recency.
//! Memories decay, can be consolidated when near-duplicate, and can be
//! linked into relationship graphs.
//!
//! The library is fully synchronous and fully concurrent: there is no
//! async runtime dependency, the entire API (except the `consolidate`
//! maintenance pass) takes `&self`, and nothing anywhere blocks on a mutex
//! or RwLock. Searches are wait-free; writes are lock-free. Share a store
//! across threads with `Arc` and use it from all of them at once.
//!
//! # Example
//!
//! ```
//! use chronomind::{ChronoMind, Config, Memory, MemoryAttributes, Vector};
//!
//! # fn main() -> chronomind::Result<()> {
//! let config = Config::builder().dimensions(4).build()?;
//! let store = ChronoMind::new(config)?;
//!
//! store.insert(Memory::new(
//!     Vector::new("first", vec![0.1, 0.2, 0.3, 0.4]),
//!     MemoryAttributes {
//!         importance: 0.8,
//!         context: "demo".into(),
//!         ..MemoryAttributes::default()
//!     },
//! ))?;
//!
//! let results = store.search(&[0.1, 0.2, 0.3, 0.4], 1)?;
//! assert_eq!(results[0].0.vector.id, "first");
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod config;
pub mod error;
pub mod index;
pub mod metric;
pub mod persistence;
pub mod store;
pub mod types;

pub use config::{Config, ConfigBuilder, IndexParams};
pub use error::{Error, Result};
pub use metric::{CosineDistance, DistanceMetric};
pub use persistence::{load_snapshot, save_snapshot};
pub use store::ChronoMind;
pub use types::{ContextSummary, Memory, MemoryAttributes, MemoryStats, Vector};
