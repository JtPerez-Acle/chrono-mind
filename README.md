# ChronoMind

<div align="center">

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](http://www.apache.org/licenses/LICENSE-2.0)
[![API Docs](https://img.shields.io/badge/docs-latest-blue.svg)](docs/API.md)
[![Benchmarks](https://img.shields.io/badge/benchmarks-view-green.svg)](docs/BENCHMARKS.md)

*A high-performance temporal vector store with advanced memory management*

[Features](#key-features) •
[Performance](#performance) •
[Getting Started](#getting-started) •
[API](#api)

</div>

## Overview

ChronoMind is a Rust-based vector similarity search engine that combines HNSW-based search with temporal awareness. It provides:

- Fast vector similarity search with verified P99 latencies
- Native temporal decay and importance weighting
- Memory-efficient storage with full temporal metadata
- Lock-free concurrent operations

## Key Features

### Vector Operations

```rust
// Initialize with custom configuration
let config = MemoryConfig {
    max_connections: 16,
    ef_construction: 100,
    ..Default::default()
};
let store = MemoryStorage::new(config)?;

// Save with temporal metadata
store.save_memory(vector).await?;

// Search with temporal awareness
let results = store.search_similar(&query, k).await?;
```

### Core Capabilities

- **HNSW-Based Search**: Multi-layer graph structure for efficient approximate nearest neighbor search
- **Temporal Awareness**: Native support for time-based memory decay and importance weighting
- **Concurrent Operations**: Lock-free architecture for parallel processing
- **Memory Efficiency**: Optimized storage with minimal overhead

## Performance

Our benchmarks are continuously updated and available in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

### Search Performance (P99)

| Dataset Size | ExactMatch | Semantic | Hybrid |
|-------------|------------|----------|---------|
| Small (10K) | 69.8µs | 1.19ms | 520.8µs |
| Medium (100K) | 279.2µs | 3.50ms | 1.79ms |

### Memory Usage

| Vector Type | Memory Per Vector |
|-------------|------------------|
| BERT (768d) | 2.8KB |
| Ada-002 (1536d) | 5.4KB |
| MiniLM (384d) | 1.2KB |

### Throughput

- **Concurrent Users**: Tested with 100 simultaneous connections
- **CPU Usage**: < 40% under full load
- **Memory Overhead**: < 10% for HNSW graph structure

## Getting Started

### Installation

#### From Source

```bash
# Clone the repository
git clone https://github.com/JtPerez-Acle/chrono-mind.git
cd chrono-mind

# Build the project
cargo build --release
```

#### As a Library

```toml
[dependencies]
vector-store = { git = "https://github.com/your-org/chrono-mind.git" }
```

### CLI Usage

```bash
# Save sample vectors
./target/release/vector-store save --input examples/sample_vectors.json --output vectors.store --dimensions 4 --normalize

# Query vectors
./target/release/vector-store query --file vectors.store --vector "[0.1, 0.2, 0.3, 0.4]" --limit 3 --normalize

# Query vectors with context filtering
./target/release/vector-store query --file vectors.store --vector "[0.1, 0.2, 0.3, 0.4]" --context "my_context" --limit 5

# Get statistics
./target/release/vector-store stats --file vectors.store
```

#### CLI Features

- **Progress Bars**: Visual feedback for large vector operations
- **Vector Normalization**: Automatically normalize vectors to unit length with `--normalize` flag
- **Context Filtering**: Filter query results by context with `--context` option
- **User-Friendly Error Messages**: Clear guidance when errors occur
- **Flexible Vector Input**: Support for both JSON array and comma-separated formats

### Library Usage

```rust
use std::sync::Arc;
use vector_store::{
    core::config::MemoryConfig,
    memory::temporal::MemoryStorage,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    storage::metrics::CosineDistance,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = MemoryConfig::default();

    // Create storage with cosine distance metric
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(config, metric);

    // Create and save a vector
    let vector = Vector::new(
        "vector1".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );

    let temporal = TemporalVector::new(
        vector,
        MemoryAttributes::default(),
    );

    // Save the vector
    storage.save_memory(temporal).await?;

    // Query similar vectors
    let query = vec![0.1, 0.2, 0.3, 0.4];
    let results = storage.search_similar(&query, 10).await?;

    for (memory, score) in results {
        println!("ID: {}, Score: {}", memory.vector.id, score);
    }

    Ok(())
}
```

## Documentation

- [User Guide](docs/USER_GUIDE.md): Detailed instructions for using ChronoMind
- [Code Review](docs/CODE_REVIEW.md): Comprehensive review of the codebase
- [Data Flow](docs/DATA_FLOW.md): Analysis of how data flows through the system
- [Future Improvements](docs/FUTURE_IMPROVEMENTS.md): Roadmap for future development
- [API Documentation](docs/API.md): Detailed API reference

### Core Components

- `MemoryStorage`: Primary interface for vector operations
- `TemporalVector`: Vector type with temporal metadata
- `MemoryBackend`: Storage backend for persistence
- `MemoryConfig`: Configuration for the vector store

### Configuration Options

```rust
pub struct MemoryConfig {
    pub max_dimensions: usize,     // Default: 768 (BERT dimensions)
    pub max_memories: usize,       // Default: 1000
    pub min_importance: f32,       // Default: 0.0
    pub max_importance: f32,       // Default: 1.0
    pub base_decay_rate: f32,      // Default: 0.1
    pub temporal_weight: f32,      // Default: 0.3
    pub similarity_threshold: f32, // Default: 0.8
    pub max_relationships: usize,  // Default: 50
    pub consolidation_window: Duration, // Default: 24 hours
    pub similar_memory_count: usize, // Default: 10
    pub max_context_window: usize, // Default: 1000
}
```

## Architecture

ChronoMind uses a multi-layer architecture:

1. **Core Layer**: HNSW-based vector index
2. **Temporal Layer**: Time-based decay and importance
3. **Concurrency Layer**: Lock-free operations

## Contributing

We welcome contributions from the community. Please be mindful of Rust's best practices and maintain a clean and readable codebase.

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## Service Comparison

When evaluating vector stores, it's essential to understand where ChronoMind fits in the ecosystem. Here's an honest comparison with similar services:

### Our Strengths

- **Temporal Features**: Native support for time-based operations and decay, which is unique among current vector stores
- **Memory Efficiency**: Our verified 2.8KB per BERT vector (768d) is competitive with industry standards
- **Lock-Free Operations**: True concurrent operations without global locks, beneficial for high-throughput scenarios
- **Rust Implementation**: Zero-cost abstractions and memory safety guarantees

### Areas for Consideration

- **Maturity**: As a newer solution, we lack the extensive production testing of established solutions like FAISS or Milvus
- **Ecosystem**: Currently fewer tools and integrations compared to more established solutions
- **Distribution**: Currently optimized for single-node deployments, while solutions like Milvus offer mature distributed architectures
- **Documentation**: While growing, our documentation and examples are not as extensive as larger projects

### When to Choose ChronoMind

Consider ChronoMind when you need:
- Time-aware vector operations with automatic decay
- High-performance single-node deployments
- Memory-efficient storage with full temporal metadata
- Rust-based implementation with strong safety guarantees

Consider alternatives when you need:
- Distributed deployments across multiple nodes
- Extensive production track record
- Large ecosystem of tools and integrations
- Complex filtering and attribute-based queries

We believe in transparency and encourage users to evaluate their specific needs against our verified capabilities.
