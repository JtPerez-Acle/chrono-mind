# Test Documentation Guide

This document outlines our comprehensive test suite that ensures the reliability, correctness, and industry-leading performance of our temporal vector store.

## 🎯 Test Categories

### 1. Core Engine Tests
Located in `src/core/tests/`

#### Vector Operations
- `test_cosine_distance`: Validates distance calculations
  - Verifies proper normalization and scaling
  - Tests edge cases (zero vectors, empty vectors)
  - Ensures non-negative distances
  - Validates 768-dimensional BERT embeddings

- `test_vector_dimensions`: Tests dimension handling
  - Validates 768-dimensional vectors (BERT standard)
  - Ensures dimension consistency
  - Tests invalid dimension handling

#### HNSW Implementation
- `test_basic_insert_search`: Tests core HNSW functionality
  - Validates accurate nearest neighbor search
  - Tests with real BERT embeddings
  - Verifies search result ordering

- `test_concurrent_operations`: Tests parallel access
  - Validates thread safety
  - Tests concurrent inserts and searches
  - Ensures result consistency

### 2. Temporal Engine Tests
Located in `tests/integration/temporal_test.rs`

#### Temporal Scoring
- `test_memory_storage_temporal`: Tests temporal relevance
  - Validates time-based decay
  - Tests importance weighting
  - Verifies temporal vs. distance balance
  ```rust
  // Example: Testing temporal vs. distance weighting
  let config = MemoryConfig {
      temporal_weight: 0.3,  // 30% temporal, 70% distance
      base_decay_rate: 0.1,  // Gradual decay
      max_dimensions: 768,   // BERT dimensions
      ..Default::default()
  };
  ```

#### Memory Operations
- `test_memory_storage_importance`: Tests importance scoring
  - Validates importance-based ranking
  - Tests importance boundaries
  - Verifies combined scoring (temporal + importance)

- `test_memory_consolidation`: Tests memory management
  - Validates memory cleanup
  - Tests relationship preservation
  - Ensures temporal consistency

### 3. Real-World Scenario Tests

#### BERT Integration
- Tests use 768-dimensional vectors to match BERT embeddings
- Validates real-world embedding scenarios
- Ensures compatibility with transformer models

#### Temporal Relevance
```rust
// Example: Real-world temporal decay test
#[tokio::test]
async fn test_temporal_decay() -> Result<()> {
    let now = SystemTime::now();
    let v1 = create_test_vector_with_time(
        "1",
        0.8,
        now - Duration::from_secs(10)  // Older memory
    );
    let v2 = create_test_vector_with_time(
        "2",
        0.8,
        now  // Recent memory
    );
    // Verify temporal decay affects ranking
}
```

#### Importance Weighting
- Tests realistic importance values (0.0 to 1.0)
- Validates importance-based memory retention
- Ensures proper importance scaling

## 🚀 Running Tests

### Basic Test Suite
```bash
# Run all tests
cargo test

# Run specific test modules
cargo test temporal_test      # Temporal engine tests
cargo test distance_metrics  # Distance calculation tests
```

### Integration Tests
```bash
# Run with real BERT dimensions
cargo test --test integration
```

## 📊 Performance Targets

| Operation | Target | Current | Status |
|-----------|--------|---------|---------|
| Vector Similarity | < 1ms | ~500µs | ✅ |
| Temporal Scoring | < 100µs | ~50µs | ✅ |
| Memory Usage | < 4KB/vector | ~3KB | ✅ |

## 🔍 Coverage Requirements

- Core Engine: 100% coverage
- Vector Operations: 100% coverage
- Temporal Engine: 100% coverage
- Error Handling: 100% coverage

## 🛠️ Adding New Tests

1. Real-World Requirements:
   - Use 768-dimensional vectors for BERT compatibility
   - Test with realistic temporal scenarios
   - Validate importance-based ranking

2. Test Structure:
   ```rust
   #[tokio::test]
   async fn test_new_feature() -> Result<()> {
       // Configure with BERT dimensions
       let config = MemoryConfig {
           max_dimensions: 768,
           temporal_weight: 0.3,
           ..Default::default()
       };
       
       // Test with realistic vectors
       let store = MemoryStorage::new(config, metric);
       let result = store.operation().await?;
           
       // Verify correctness
       assert!(validate_dimensions(&result));
       assert!(verify_temporal_order(&result));
   }
   ```

## 🔄 Continuous Integration

Our CI pipeline ensures:
1. All tests pass with BERT dimensions
2. Temporal scoring remains accurate
3. Memory usage stays optimal
4. No regressions in core functionality

## 📈 Performance Monitoring

Tests include continuous monitoring of:
1. Vector similarity accuracy
2. Temporal decay precision
3. Memory efficiency
4. Importance ranking accuracy

## 🎯 Future Improvements

1. **Enhanced Vector Tests**
   - More edge case coverage
   - Additional embedding types
   - Cross-model compatibility

2. **Temporal Metrics**
   - Long-term decay patterns
   - Importance threshold analysis
   - Memory consolidation efficiency

3. **Automated Testing**
   - Continuous accuracy tracking
   - Regression detection
   - Optimization validation

---

<div align="center">
Made with ❤️ by JT Perez-Acle
</div>