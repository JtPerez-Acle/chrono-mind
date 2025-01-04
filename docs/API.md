# Vector Store API Documentation

## Core Components

### MemoryStorage

The primary interface for vector storage operations with temporal awareness.

```rust
use vector_store::memory::MemoryStorage;
```

#### Initialization

```rust
// Initialize with default configuration
let store = MemoryStorage::new(CosineDistance::default(), MemoryConfig::default())?;

// Initialize with custom configuration
let config = MemoryConfig {
    max_connections: 16,
    ef_construction: 100,
    ..Default::default()
};
let store = init_with_config(config)?;
```

#### Core Operations

##### Save Memory
```rust
pub async fn save_memory(&self, memory: TemporalVector) -> Result<()>
```
Saves a temporal vector to storage with automatic relationship tracking.

##### Search Similar
```rust
pub async fn search_similar(
    &self, 
    query: &[f32], 
    k: usize
) -> Result<Vec<(TemporalVector, f32)>>
```
Finds k-nearest neighbors using temporal-aware similarity search.

##### Get Related Memories
```rust
pub async fn get_related_memories(
    &self,
    id: &str,
    max_depth: usize
) -> Result<Vec<TemporalVector>>
```
Retrieves related memories up to specified relationship depth.

##### Context Operations
```rust
pub async fn search_by_context(
    &self,
    context: &str,
    query: &[f32],
    k: usize
) -> Result<Vec<(TemporalVector, f32)>>

pub async fn get_context_summary(
    &self,
    context: &str
) -> Result<Option<ContextSummary>>
```

##### Memory Management
```rust
pub async fn cleanup_old_memories(&self) -> Result<()>
pub async fn consolidate_context_memories(&self, memories: &[TemporalVector]) -> Result<()>
pub async fn compress_memories(&self, context: &str) -> Result<Vec<TemporalVector>>
```

### TemporalHNSW

Hierarchical Navigable Small World implementation with temporal awareness.

```rust
use vector_store::storage::hnsw::TemporalHNSW;
```

#### Core Operations

##### Insert
```rust
pub fn insert(&self, temporal: &TemporalVector) -> Result<()>
```
Inserts a temporal vector into the HNSW index.

##### Search
```rust
pub fn search(
    &self,
    query: &[f32],
    k: usize
) -> Result<Vec<(String, f32)>>
```
Performs k-NN search in the HNSW index.

## Data Types

### Vector
```rust
pub struct Vector {
    pub id: String,
    pub data: Vec<f32>,
}
```

### TemporalVector
```rust
pub struct TemporalVector {
    pub vector: Vector,
    pub attributes: MemoryAttributes,
}

impl TemporalVector {
    pub fn new(vector: Vector, attributes: MemoryAttributes) -> Self
    pub fn validate(&self) -> bool
    pub fn update_access(&mut self)
    pub fn get_age(&self) -> Duration
    pub fn get_last_access_age(&self) -> Duration
}
```

### MemoryAttributes
```rust
pub struct MemoryAttributes {
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub access_count: u32,
    pub context: String,
    pub metadata: HashMap<String, String>,
}
```

## Configuration

### MemoryConfig
```rust
pub struct MemoryConfig {
    pub max_connections: usize,      // Maximum connections per node
    pub ef_construction: usize,      // Size of dynamic candidate list
    pub ef_search: usize,           // Size of dynamic candidate list during search
    pub cleanup_interval: Duration,  // Interval for memory cleanup
    pub decay_factor: f32,          // Memory decay rate
}
```

## Performance Monitoring

### MetricsRegistry
```rust
pub struct MetricsRegistry {
    // Internal metrics collection
}

impl MetricsRegistry {
    pub fn record_operation_duration(&self, operation: &str, duration: Duration)
    pub fn record_memory_usage(&self, bytes: u64, context: &str)
    pub fn record_vector_operation(&self, operation_type: &str)
}
```

### PerformanceMonitor
```rust
pub struct PerformanceMonitor {
    // Performance monitoring
}

impl PerformanceMonitor {
    pub fn new(name: &str, metrics: Arc<MetricsRegistry>) -> Self
    pub fn record_metric(&self, value: f64, attributes: &[KeyValue])
}
```

## Error Handling

All operations return a `Result<T, MemoryError>` where `MemoryError` can be:

```rust
pub enum MemoryError {
    InvalidDimensions(usize, usize),
    InvalidVector(String),
    StorageError(String),
    NotFound(String),
    TaskError(String),
    Other(String),
}
```

## Performance Characteristics

- Search Complexity: O(log N) average case
- Insert Complexity: O(log N) average case
- Memory Usage: O(N) where N is number of vectors
- Vector Normalization: Automatic
- Thread Safety: All operations are thread-safe

## Best Practices

1. **Vector Dimensions**
   - Keep consistent across operations
   - Normalize vectors before insertion
   - Typical dimension range: 64-1024

2. **Memory Management**
   - Regular cleanup_old_memories() calls
   - Monitor memory usage with MetricsRegistry
   - Use context-based organization

3. **Search Optimization**
   - Adjust ef_search for quality/speed tradeoff
   - Use appropriate max_connections
   - Consider temporal aspects in search

4. **Performance Monitoring**
   - Track operation durations
   - Monitor memory usage
   - Watch cache hit ratios

## Example Usage

```rust
use vector_store::{
    init_with_config,
    memory::{MemoryStorage, TemporalVector, Vector},
    core::config::MemoryConfig,
};

async fn example() -> Result<()> {
    // Initialize storage
    let config = MemoryConfig::default();
    let store = init_with_config(config)?;
    
    // Create and save a vector
    let vector = Vector::new(
        "vec1".to_string(),
        vec![0.1, 0.2, 0.3]
    );
    let temporal = TemporalVector::new(
        vector,
        MemoryAttributes::default()
    );
    store.save_memory(temporal).await?;
    
    // Search similar vectors
    let query = vec![0.15, 0.25, 0.35];
    let results = store.search_similar(&query, 10).await?;
    
    Ok(())
}
```

## Benchmarking

The crate includes a comprehensive benchmark suite:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark groups
cargo bench memory_operations
cargo bench temporal_operations
cargo bench hnsw_operations
```

Performance targets:
- Latency: P99 < 10ms
- Throughput: 1000+ QPS
- Recall@10: > 0.95
- Memory overhead: < 100 bytes per vector