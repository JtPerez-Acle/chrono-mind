# Vector Store

[![Crates.io](https://img.shields.io/crates/v/vector-store.svg)](https://crates.io/crates/vector-store)
[![Documentation](https://docs.rs/vector-store/badge.svg)](https://docs.rs/vector-store)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://github.com/vector-store/vector-store/workflows/Rust/badge.svg)](https://github.com/vector-store/vector-store/actions)

A high-performance, temporal-aware vector storage implementation written in Rust, designed for efficient similarity search using Hierarchical Navigable Small World (HNSW) graphs.

## Features

- 🚀 High-performance vector similarity search using HNSW algorithm
- ⏱️ Temporal-aware vector storage with importance decay
- 💾 Efficient memory-mapped storage for large vector datasets
- 🔄 Asynchronous API with Tokio runtime
- 📊 Built-in tracing and metrics for observability
- 🛡️ Robust error handling with thiserror and anyhow
- 🧪 Comprehensive test suite with property-based testing

## Requirements

- Rust 2021 edition or later
- Tokio async runtime
- Compatible with Linux, macOS, and Windows

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
vector-store = "0.1.0"
tokio = { version = "1.35", features = ["full"] }
```

## Quick Start

```rust
use vector_store::{
    TemporalHNSW, HNSWConfig, CosineDistance,
    TemporalVector, MemoryAttributes,
};
use tokio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure HNSW index with temporal awareness
    let config = HNSWConfig::default()
        .with_dimensions(128)
        .with_max_connections(16)
        .with_ef_construction(100)
        .with_temporal_weight(0.5);

    // Create a new temporal-aware HNSW index
    let metric = CosineDistance::new();
    let index = TemporalHNSW::new(metric, config);

    // Create and insert a temporal vector
    let vector = TemporalVector::new(
        vec![0.1; 128],
        1.0, // importance
    )?;

    index.insert(&vector).await?;

    // Search with both similarity and temporal aspects
    let results = index.search(&vec![0.1; 128], 5).await?;

    Ok(())
}
```

## Project Structure

```
vector-store/
├── src/
│   ├── hnsw/       # HNSW implementation
│   ├── storage/    # Storage backends
│   ├── temporal/   # Temporal vector logic
│   └── utils/      # Utility functions
├── tests/
│   ├── integration/# Integration tests
│   │   ├── hnsw_test.rs
│   │   └── temporal_test.rs
│   └── utils/      # Test utilities
└── benches/        # Performance benchmarks
```

## Testing

The project includes a comprehensive test suite:

```bash
# Run all tests with logging
cargo test --all-features

# Run specific test suites
cargo test --test integration  # Integration tests
cargo test temporal           # Temporal-related tests
cargo test hnsw              # HNSW-related tests

# Run tests with logging output
RUST_LOG=debug cargo test
```

### Property-Based Testing

We use `proptest` for property-based testing to ensure robustness:

```bash
cargo test --test proptest
```

## Benchmarking

Performance benchmarks using Criterion:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench vector_ops
```

## Configuration

### HNSW Parameters

```rust
HNSWConfig {
    dimensions: usize,           // Vector dimensions
    max_connections: usize,      // Maximum connections per node
    ef_construction: usize,      // Size of dynamic candidate list
    level_multiplier: f64,       // Controls number of layers
    temporal_weight: f32,        // Weight of temporal score (0.0 - 1.0)
}
```

## Error Handling

We use `thiserror` for error definitions and `anyhow` for error propagation:

```rust
use vector_store::Result;

fn my_function() -> Result<()> {
    // Operations that may fail
    Ok(())
}
```

## Logging and Metrics

Built-in tracing support using the `tracing` crate:

```rust
use tracing::{info, error, debug};

// Enable logging
tracing_subscriber::fmt::init();
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- HNSW algorithm implementation inspired by the original paper
- Built with Rust and Tokio async runtime
- Special thanks to all contributors
