# Vector Store

A high-performance vector storage implementation written in Rust, designed for efficient similarity search using Hierarchical Navigable Small World (HNSW) graphs.

## Features

- 🚀 High-performance vector similarity search using HNSW algorithm
- 💾 Efficient memory-mapped storage for large vector datasets
- 🔄 Asynchronous API for concurrent operations
- 📊 Built-in metrics for performance monitoring
- 🛡️ Robust error handling and data integrity checks
- 🧪 Comprehensive test suite with property-based testing

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
vector-store = "0.1.0"
```

## Quick Start

```rust
use vector_store::storage::hnsw::{HNSWConfig, HNSW};
use vector_store::storage::metrics::Metric;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure HNSW index
    let config = HNSWConfig::default()
        .with_dimensions(128)
        .with_max_connections(16)
        .with_ef_construction(100);

    // Create a new HNSW index
    let mut index = HNSW::new(config)?;

    // Insert vectors
    let vector = vec![0.1; 128];
    index.insert(vector).await?;

    // Search for similar vectors
    let query = vec![0.1; 128];
    let results = index.search(&query, 5).await?;

    Ok(())
}
```

## Architecture

The project is organized into several key components:

### Core Components

- **HNSW Implementation** (`storage/hnsw/`):
  - Graph-based approximate nearest neighbor search
  - Efficient multi-layer navigation structure
  - Configurable parameters for performance tuning

- **Storage Backend** (`storage/`):
  - Memory-mapped file storage for vector data
  - In-memory storage for testing and small datasets
  - Flexible storage interface for future implementations

- **Metrics** (`storage/metrics.rs`):
  - Distance calculations (Euclidean, Cosine, Dot Product)
  - Performance monitoring and statistics

### Performance

The implementation is optimized for:
- Fast vector insertion with concurrent operations
- Efficient similarity search using HNSW algorithm
- Memory efficiency through memory-mapped files
- Cache-friendly data structures

## Configuration

### HNSW Parameters

```rust
HNSWConfig {
    dimensions: usize,           // Vector dimensions
    max_connections: usize,      // Maximum connections per node
    ef_construction: usize,      // Size of dynamic candidate list
    level_multiplier: f64,       // Controls number of layers
}
```

### Storage Options

- **Memory-Mapped Storage**: Efficient for large datasets
- **In-Memory Storage**: Suitable for testing and small datasets

## Development

### Prerequisites

- Rust 1.70 or higher
- Cargo

### Building

```bash
cargo build --release
```

### Running Tests

```bash
# Run unit tests
cargo test

# Run benchmarks
cargo bench
```

### Benchmarking

The project includes benchmarks for:
- Vector insertion performance
- Search operation latency
- Memory usage patterns

Run benchmarks using:
```bash
cargo bench
```

## Error Handling

The library uses custom error types for detailed error handling:
- Storage-related errors
- Configuration validation
- Runtime errors with detailed context

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Guidelines
1. Write tests for new functionality
2. Update documentation for public APIs
3. Follow Rust best practices and idioms
4. Run the full test suite before submitting PRs

## Implementation Details

### Vector Distance Metrics

The library supports multiple distance metrics for vector similarity:

- **Euclidean Distance**: L2 norm-based distance calculation
  ```rust
  // Example usage
  use vector_store::storage::metrics::EuclideanDistance;
  let metric = EuclideanDistance;
  let distance = metric.distance(&vec1, &vec2);
  ```

- **Cosine Distance**: Angular distance between vectors
  ```rust
  use vector_store::storage::metrics::CosineDistance;
  let metric = CosineDistance;
  let distance = metric.distance(&vec1, &vec2);
  ```

- **Dot Product Distance**: Inner product-based similarity
  ```rust
  use vector_store::storage::metrics::DotProductDistance;
  let metric = DotProductDistance;
  let distance = metric.distance(&vec1, &vec2);
  ```

Custom metrics can be implemented by implementing the `DistanceMetric` trait:
```rust
pub trait DistanceMetric: Send + Sync + Debug {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;
    fn name(&self) -> &'static str;
}
```

### HNSW Graph Structure

The HNSW index is implemented as a hierarchical graph with the following properties:

- **Multi-layer Structure**: Vectors are organized in layers, with fewer nodes in higher layers
- **Skip-list-like Navigation**: Fast approximate search through layer-wise traversal
- **Dynamic Construction**: Efficient online index building with concurrent operations
- **Configurable Connectivity**: Adjustable node connections for performance tuning

### Advanced Usage

#### Custom Configuration

```rust
use vector_store::storage::hnsw::{Config, HnswIndex};
use vector_store::storage::metrics::EuclideanDistance;

let config = Config {
    max_layers: 16,             // Number of layers in the graph
    ef_construction: 100,       // Size of dynamic candidate list during construction
    max_connections: 16,        // Maximum connections per node
    extend_candidates: true,    // Enable candidate list extension
    keep_pruned: true,         // Keep pruned connections as candidates
};

let index = HnswIndex::new(
    config,
    Box::new(EuclideanDistance)
);
```

#### Batch Operations

For better performance with large datasets:

```rust
use vector_store::storage::VectorBatch;

async fn batch_insert(index: &mut HnswIndex, vectors: Vec<Vector>) -> Result<()> {
    let batch = VectorBatch::new(vectors);
    index.batch_insert(batch).await
}
```

#### Memory Management

The library provides different storage backends:

1. **Memory-Mapped Files**:
```rust
use vector_store::storage::mmap::MmapStorage;

let storage = MmapStorage::new("vectors.db")?;
```

2. **In-Memory Storage**:
```rust
use vector_store::storage::memory::MemoryStorage;

let storage = MemoryStorage::new();
```

### Performance Optimization

#### Index Building

- Use appropriate `ef_construction` values:
  - Higher values (100-200) for better accuracy
  - Lower values (40-50) for faster construction
- Adjust `max_connections` based on dimensionality:
  - Higher dimensions may benefit from more connections
  - Lower dimensions work well with fewer connections

#### Search Optimization

- Tune `ef_search` parameter for search:
  - Higher values increase accuracy but slow down search
  - Lower values provide faster but approximate results
- Use batch search for multiple queries:
  ```rust
  let results = index.batch_search(&queries, 10, 100).await?;
  ```

### Monitoring and Metrics

The library provides built-in monitoring capabilities:

```rust
use vector_store::metrics::{Metrics, MetricsCollector};

// Enable metrics collection
let metrics = MetricsCollector::new();
index.set_metrics_collector(metrics.clone());

// Get performance statistics
let stats = metrics.get_statistics();
println!("Average search time: {}ms", stats.avg_search_time);
println!("Index size: {} bytes", stats.index_size);
```

### Error Handling

The library uses a custom error type for detailed error handling:

```rust
use vector_store::error::VectorStoreError;

match result {
    Err(VectorStoreError::DimensionMismatch { expected, got }) => {
        println!("Vector dimension mismatch: expected {}, got {}", expected, got);
    }
    Err(VectorStoreError::StorageError(err)) => {
        println!("Storage error: {}", err);
    }
    // ... handle other error types
}
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- HNSW algorithm paper: "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs" by Yu. A. Malkov, D. A. Yashunin
- The Rust community for excellent tools and crates
