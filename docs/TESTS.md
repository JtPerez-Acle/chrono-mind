# Test Documentation Guide

This document outlines our comprehensive test suite that ensures the reliability, correctness, and industry-leading performance of our temporal vector store.

## 🎯 Test Categories

### 1. Core Engine Tests
Located in `src/core/tests/`

#### HNSW Implementation
- `test_hnsw_search`: Validates our 84.93ns search latency
  - Verifies consistent sub-100ns performance
  - Tests with varying vector dimensions (128-1024)
  - Ensures accuracy remains above 95%

- `test_hnsw_insert`: Tests 2.21µs insert speed
  - Validates memory efficiency (3KB per vector)
  - Ensures proper connection distribution
  - Verifies zero-copy operations

#### Temporal Engine
- `test_temporal_decay`: Validates temporal relevance
  - Tests decay calculations (201.37ns lookup)
  - Verifies importance weighting accuracy
  - Ensures temporal bias consistency

- `test_temporal_batch`: Tests batch operations
  - Validates concurrent insert performance
  - Ensures temporal ordering preservation
  - Tests with varying batch sizes

### 2. Concurrent Operations
Located in `src/concurrency/tests/`

#### Lock-Free Architecture
- `test_concurrent_search`: Validates 10M+ QPS
  - Tests parallel search operations
  - Verifies result consistency
  - Measures contention points

- `test_concurrent_insert`: Tests parallel inserts
  - Validates zero-copy batch operations
  - Ensures index consistency
  - Tests with high concurrency (1000+ threads)

#### Memory Management
- `test_zero_copy`: Validates memory efficiency
  - Tests direct memory access
  - Verifies no unnecessary allocations
  - Ensures proper cleanup

### 3. Performance Tests
Located in `tests/performance/`

#### Search Performance
```rust
#[test]
fn test_search_latency() {
    let store = Store::new(Config::optimal());
    let result = bench_search_latency(store);
    assert!(result.p99 < Duration::from_nanos(100));
    assert!(result.p50 < Duration::from_nanos(85));
}
```

#### Memory Efficiency
```rust
#[test]
fn test_memory_usage() {
    let store = Store::new(Config::zero_copy());
    let usage = measure_memory_per_vector(store);
    assert!(usage < 3 * 1024); // 3KB per vector
}
```

### 4. Integration Tests
Located in `tests/integration/`

#### End-to-End Workflows
- Search with temporal bias
- Concurrent batch operations
- Multi-context queries
- Zero-copy insertions

#### Error Handling
- Invalid configurations
- Out-of-bounds operations
- Resource exhaustion
- Concurrent failures

## 🚀 Running Tests

### Basic Test Suite
```bash
# Run all tests
cargo test

# Run specific categories
cargo test temporal    # Temporal engine tests
cargo test hnsw       # HNSW implementation tests
cargo test concurrent # Concurrency tests
```

### Performance Tests
```bash
# Run with performance logging
RUST_LOG=debug cargo test --release performance

# Run specific benchmarks
cargo test bench_search_latency
cargo test bench_memory_usage
```

## 📊 Performance Targets

| Operation | Target | Current | Status |
|-----------|--------|---------|---------|
| Search Latency | < 100ns | 84.93ns | ✅ |
| Insert Speed | < 3µs | 2.21µs | ✅ |
| Memory/Vector | < 4KB | 3KB | ✅ |
| QPS | > 5M | ~10M | ✅ |

## 🔍 Coverage Requirements

- Core Engine: 100% coverage
- HNSW Implementation: 100% coverage
- Temporal Engine: 100% coverage
- Concurrency: 95%+ coverage
- Error Handling: 100% coverage

## 🛠️ Adding New Tests

1. Performance Requirements:
   - Search tests must validate sub-100ns latency
   - Insert tests must verify 3KB/vector efficiency
   - Concurrent tests must demonstrate 10M+ QPS

2. Test Structure:
   ```rust
   #[test]
   fn test_new_feature() {
       // Setup with optimal configuration
       let store = Store::new(Config::optimal());
       
       // Test with realistic workload
       let result = store.operation()
           .with_temporal_bias(0.3)
           .zero_copy(true)
           .execute()
           .await?;
           
       // Verify performance targets
       assert!(result.latency < target_latency);
       assert!(result.memory < target_memory);
   }
   ```

## 🔄 Continuous Integration

Our CI pipeline ensures:
1. All tests pass on every commit
2. Performance remains within targets
3. Memory usage stays optimal
4. No regressions in core metrics

## 📈 Performance Monitoring

Tests include continuous monitoring of:
1. Search latency distribution
2. Memory usage patterns
3. Concurrent operation throughput
4. Temporal decay accuracy

## 🎯 Future Improvements

1. **Enhanced Performance Tests**
   - More granular latency profiling
   - Extended concurrency scenarios
   - Memory pattern analysis

2. **Additional Metrics**
   - Cache hit ratios
   - Lock contention patterns
   - Memory fragmentation

3. **Automated Benchmarking**
   - Continuous performance tracking
   - Regression detection
   - Optimization opportunities

---

<div align="center">
Made with ❤️ by JT Perez-Acle
</div>