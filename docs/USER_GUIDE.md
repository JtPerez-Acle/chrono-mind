# ChronoMind User Guide

ChronoMind is a high-performance temporal vector store designed for AI applications. This guide will help you get started with using ChronoMind in your projects.

## Installation

### From Source

To build ChronoMind from source, you'll need Rust installed on your system. If you don't have Rust installed, you can get it from [rustup.rs](https://rustup.rs/).

```bash
# Clone the repository
git clone https://github.com/your-org/chrono-mind.git
cd chrono-mind

# Build the project
cargo build --release

# The binary will be available at target/release/vector-store
```

## Basic Usage

ChronoMind provides a command-line interface for basic operations. Here are some examples:

### Saving Vectors

You can save vectors to a file using the `save` command:

```bash
# Save a single vector from a JSON file
vector-store save --input vector.json --output vectors.store --dimensions 4

# The input file should be in JSON format:
# {
#   "id": "vector1",
#   "data": [0.1, 0.2, 0.3, 0.4],
#   "importance": 0.8,
#   "context": "test_context",
#   "decay_rate": 0.1
# }
```

You can also save multiple vectors at once by providing an array of vectors in the input file:

```json
[
  {
    "id": "vector1",
    "data": [0.1, 0.2, 0.3, 0.4],
    "importance": 0.8,
    "context": "context_a",
    "decay_rate": 0.1
  },
  {
    "id": "vector2",
    "data": [0.2, 0.3, 0.4, 0.5],
    "importance": 0.6,
    "context": "context_b",
    "decay_rate": 0.2
  }
]
```

### Querying Vectors

You can query vectors using the `query` command:

```bash
# Find similar vectors
vector-store query --file vectors.store --vector "[0.1, 0.2, 0.3, 0.4]" --limit 5

# Filter by context
vector-store query --file vectors.store --vector "[0.1, 0.2, 0.3, 0.4]" --context "context_a" --limit 5
```

### Getting Statistics

You can get statistics about your stored vectors using the `stats` command:

```bash
vector-store stats --file vectors.store
```

## Advanced Usage

### Using ChronoMind as a Library

ChronoMind can be used as a library in your Rust projects. Add it to your `Cargo.toml`:

```toml
[dependencies]
vector-store = { git = "https://github.com/your-org/chrono-mind.git" }
```

Here's a simple example of using ChronoMind in your code:

```rust
use std::sync::Arc;
use vector_store::{
    core::config::MemoryConfig,
    memory::temporal::MemoryStorage,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    storage::metrics::CosineDistance,
};
use std::time::SystemTime;

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
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 0.8,
            context: "test_context".to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
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

### Temporal Features

ChronoMind's key feature is its temporal awareness. Vectors have importance values that decay over time, making more recent and important memories more likely to be retrieved.

You can control this behavior through several parameters:

- `importance`: How important a memory is (0.0 to 1.0)
- `decay_rate`: How quickly the importance decays over time
- `context`: A tag for grouping related memories
- `relationships`: Links to other related memories

### Performance Considerations

For optimal performance:

1. Use appropriate vector dimensions for your use case
2. Normalize vectors before storing them
3. Use contexts to partition your vector space
4. Set appropriate decay rates based on your application needs

## API Reference

For detailed API documentation, run:

```bash
cargo doc --open
```

## Troubleshooting

### Common Issues

1. **Invalid dimensions error**: Make sure the dimensions of your vectors match the `max_dimensions` in your configuration.

2. **File not found**: Check that the file paths you're providing are correct.

3. **Performance issues**: For large vector collections, consider increasing the `max_memories` parameter and using more specific contexts.

## Getting Help

If you encounter any issues or have questions, please file an issue on the GitHub repository.

## Contributing

Contributions are welcome! Please see the [CONTRIBUTING.md](CONTRIBUTING.md) file for guidelines.
