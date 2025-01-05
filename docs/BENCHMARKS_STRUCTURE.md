# ChronoMind Benchmark Structure Documentation

This document outlines the structure and methodology of our benchmark suite, designed to validate ChronoMind's industry-leading performance claims.

## Core Benchmarks

### 1. HNSW Operations
Located in `benches/hnsw/mod.rs`

#### Configuration Profiles
```rust
const HNSW_CONFIGS: [(usize, usize, usize); 3] = [
    // M, ef_construction, ef_search - tuned for different scenarios
    (16, 100, 50),   // Fast search (high QPS)
    (32, 200, 100),  // Balanced (default)
    (48, 400, 200),  // High accuracy
];
```

- **Fast Search Profile**
  - Target: Maximum QPS
  - M = 16: Minimal connections for fast traversal
  - ef_construction = 100: Quick index construction
  - ef_search = 50: Fast search with acceptable recall

- **Balanced Profile**
  - Target: Production default
  - M = 32: Good balance of speed and recall
  - ef_construction = 200: Quality index construction
  - ef_search = 100: Balanced search parameters

- **High Accuracy Profile**
  - Target: Maximum recall
  - M = 48: Dense connections for thorough search
  - ef_construction = 400: High-quality index
  - ef_search = 200: Thorough search for best results

### 2. Memory Operations
Located in `benches/memory/mod.rs`

#### Embedding Model Support
```rust
// Model dimensions based on popular embedding models
pub mod config {
    pub const DIMS_BERT_BASE: usize = 768;    // BERT base model
    pub const DIMS_ADA_002: usize = 1536;     // OpenAI ada-002
    pub const DIMS_E5_LARGE: usize = 1024;    // E5 large model
    pub const DIMS_MINILM: usize = 384;       // MiniLM model
}
```

- **Dataset Scales**
  - Small: 10K vectors (rapid testing)
  - Medium: 100K vectors (standard benchmark)
  - Large: 1M vectors (production scale)

- **Memory Importance Levels**
  ```rust
  pub const IMPORTANCE_CONFIGS: [(f32, &str); 4] = [
      (1.0, "critical"),     // High importance, immediate recall
      (0.8, "important"),    // Important but not critical
      (0.5, "normal"),       // Standard importance
      (0.2, "background"),   // Background/archival data
  ];
  ```

### 3. Query Patterns
Located in `benches/common/mod.rs`

#### Search Scenarios
```rust
pub enum QueryPattern {
    ExactMatch,   // Exact content retrieval
    Semantic,     // Semantic similarity search
    Hybrid,       // Combination of exact and semantic
}
```

- **Metrics Tracked**
  ```rust
  pub enum MetricType {
      Latency,    // Response time in milliseconds
      Throughput, // Queries per second
      Accuracy,   // Precision@K for search results
      Memory,     // Memory usage in MB
  }
  ```

## Benchmark Output Format

### Performance Metrics
```json
{
  "model": "bert-base",
  "config": "balanced",
  "dataset_size": 100000,
  "metrics": {
    "latency": {
      "p50": "84.93ns",
      "p95": "92.15ns",
      "p99": "98.72ns"
    },
    "throughput": {
      "qps": 10000000,
      "concurrent_users": 100
    },
    "accuracy": {
      "recall@10": 0.95,
      "precision@10": 0.92
    },
    "memory": {
      "per_vector": "3KB",
      "total_usage": "305MB",
      "index_overhead": "20%"
    }
  }
}
```

### Temporal Performance
```json
{
  "decay_calculation": {
    "latency": "201.37ns",
    "accuracy": 0.98
  },
  "context_search": {
    "latency": "15.26µs",
    "recall": 0.94
  }
}
```

## Understanding Results

### 1. Latency Interpretation
- **P50 (Median)**: Normal operation latency
- **P95**: Slight degradation threshold
- **P99**: Worst-case performance

### 2. Throughput Analysis
- **QPS**: Raw queries per second
- **Concurrent Users**: Scalability metric
- **Batch Performance**: Efficiency at scale

### 3. Accuracy Metrics
- **Recall@K**: Proportion of relevant items found
- **Precision@K**: Proportion of found items that are relevant
- **F1 Score**: Harmonic mean of precision and recall

### 4. Memory Efficiency
- **Per Vector**: Raw storage cost
- **Index Overhead**: Additional HNSW structure cost
- **Total Usage**: Full system memory footprint

## Running Benchmarks

```bash
# Full benchmark suite
cargo bench

# Specific benchmark group
cargo bench --bench memory_ops
cargo bench --bench hnsw_ops
cargo bench --bench temporal_ops

# With specific configuration
RUST_LOG=info cargo bench -- --verbose
```

## Visualization Tools

Results are automatically generated in HTML format:
- Located in `target/criterion/report/index.html`
- Interactive graphs and comparisons
- Historical performance tracking
- Regression detection

## Performance Targets (2025-Q1)

| Operation | Target | Validation Method |
|-----------|---------|------------------|
| Search Latency | < 100ns | P99 latency under load |
| Insert Speed | < 3µs | Single vector insertion |
| Memory/Vector | < 4KB | Total size / vector count |
| QPS | > 5M | Sustained throughput |
| Recall@10 | > 0.95 | Standard test set |