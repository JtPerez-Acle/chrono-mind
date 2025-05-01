# ChronoMind Developer Quick Start Guide

This guide will help you get started with developing ChronoMind quickly. It provides an overview of the codebase structure, development workflow, and key concepts.

## Setting Up Your Development Environment

### Prerequisites

- Rust (1.75 or later)
- Cargo
- Git

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/your-org/chrono-mind.git
cd chrono-mind

# Build the project
cargo build

# Run tests
cargo test
```

## Codebase Structure

ChronoMind is organized into several modules:

```
src/
├── core/           # Core configuration and error handling
│   ├── config.rs   # Configuration structures
│   └── error.rs    # Error types and handling
├── memory/         # Temporal memory implementation
│   ├── temporal.rs # Main memory storage implementation
│   └── types.rs    # Memory-related data structures
├── storage/        # Storage backends
│   ├── hnsw.rs     # HNSW index for similarity search
│   ├── metrics.rs  # Distance metrics (cosine, etc.)
│   └── persistence.rs # Persistence layer
├── utils/          # Utility functions
│   ├── monitoring.rs # Performance monitoring
│   └── validation.rs # Input validation
└── main.rs         # CLI implementation
```

## Key Concepts

### Temporal Vectors

The core concept in ChronoMind is the `TemporalVector`, which extends a regular vector with temporal attributes:

```rust
pub struct Vector {
    pub id: String,
    pub data: Vec<f32>,
}

pub struct TemporalVector {
    pub vector: Vector,
    pub attributes: MemoryAttributes,
}

pub struct MemoryAttributes {
    pub timestamp: SystemTime,
    pub importance: f32,
    pub context: String,
    pub decay_rate: f32,
    pub relationships: Vec<String>,
    pub access_count: usize,
    pub last_access: SystemTime,
}
```

### Memory Storage

The `MemoryStorage` class is the main interface for storing and retrieving vectors:

```rust
// Create a new memory storage
let config = MemoryConfig::default();
let metric = Arc::new(CosineDistance::new());
let storage = MemoryStorage::new(config, metric);

// Save a vector
storage.save_memory(temporal_vector).await?;

// Search for similar vectors
let results = storage.search_similar(&query_vector, 10).await?;

// Search by context
let results = storage.search_by_context("context_name", &query_vector, 10).await?;
```

### Persistence

The persistence layer allows saving and loading vectors to/from disk:

```rust
// Create a backend
let config = MemoryConfig::default();
let mut backend = MemoryBackend::new(config);

// Save vectors
backend.save(&temporal_vector).await?;

// Backup to file
backend.backup(path).await?;

// Restore from file
backend.restore(path).await?;
```

## Development Workflow

We follow a Test-Driven Development (TDD) approach:

1. **Write Tests First**: Create tests that define the expected behavior
2. **Implement Features**: Write code to make the tests pass
3. **Refactor**: Improve the code while keeping tests passing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific tests
cargo test --test integration

# Run with logging
RUST_LOG=debug cargo test
```

### Using the CLI for Testing

The CLI interface is useful for manual testing:

```bash
# Save sample vectors
cargo run -- save --input examples/sample_vectors.json --output vectors.store --dimensions 4

# Query vectors
cargo run -- query --file vectors.store --vector "[0.1, 0.2, 0.3, 0.4]" --limit 3

# Get statistics
cargo run -- stats --file vectors.store
```

## Common Development Tasks

### Adding a New Feature

1. Create tests in the appropriate test file
2. Implement the feature
3. Update documentation
4. Run all tests to ensure nothing breaks

### Fixing a Bug

1. Create a test that reproduces the bug
2. Fix the implementation
3. Verify the test passes
4. Add regression tests if needed

### Improving Performance

1. Create a benchmark or use existing ones
2. Measure current performance
3. Implement optimizations
4. Measure again to verify improvement

## Debugging Tips

### Enabling Logging

ChronoMind uses the `tracing` crate for logging:

```bash
# Set log level
RUST_LOG=debug cargo run -- [commands]

# For more detailed logs
RUST_LOG=trace cargo run -- [commands]
```

### Common Issues

1. **Dimension Mismatch**: Ensure vector dimensions match the configuration
2. **Memory Usage**: For large vectors, monitor memory usage
3. **Concurrency Issues**: Use `RUST_BACKTRACE=1` to get detailed backtraces

## Code Style and Conventions

- Use Rust's standard formatting (`cargo fmt`)
- Follow Rust's naming conventions
- Document public APIs with doc comments
- Keep functions small and focused
- Use meaningful variable names

## Next Steps

Check the [TODO.md](../TODO.md) file for a list of tasks that need to be addressed. Pick one that interests you and start contributing!

For more detailed information, refer to:
- [User Guide](USER_GUIDE.md)
- [Code Review](CODE_REVIEW.md)
- [Data Flow](DATA_FLOW.md)
- [Future Improvements](FUTURE_IMPROVEMENTS.md)

Happy coding!
