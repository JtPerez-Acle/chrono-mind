# Test Documentation Guide

This document provides a comprehensive overview of the test suite in our vector store implementation. The tests are designed to ensure reliability, correctness, and performance of our temporal-aware vector similarity search system.

## 🎯 Test Categories

### 1. Core Tests
Located in `src/core/tests/`

#### Configuration Tests
- `test_default_config`: Validates default configuration values
- `test_custom_config`: Ensures custom configurations are properly applied
- `test_invalid_config`: Verifies proper error handling for invalid configurations

### 2. Storage Tests
Located in `src/storage/tests/` and `tests/integration/hnsw_test.rs`

#### HNSW Implementation Tests
- `test_basic_insert_search`: Validates core HNSW functionality
  - Tests insertion and retrieval of vectors
  - Verifies correct distance-based ordering (temporal weight: 0.1)
  - Ensures proper handling of vector dimensions

- `test_temporal_ordering`: Tests temporal aspects of search
  - Uses high temporal weight (0.8) to prioritize recent items
  - Verifies that recent items are ranked higher despite lower similarity
  - Validates temporal decay calculations

- `test_empty_index`: Ensures proper behavior with empty index
  - Verifies correct handling of searches on empty index
  - Validates error handling for empty states

- `test_dimension_validation`: Tests dimension validation logic
  - Ensures vectors match configured dimensions
  - Validates error handling for mismatched dimensions

- `test_concurrent_operations`: Validates thread safety
  - Tests parallel insertions and searches
  - Verifies data consistency under concurrent access
  - Ensures proper lock handling

- `test_layer_stats`: Validates HNSW structure
  - Tests layer generation and connections
  - Verifies node distribution across layers
  - Ensures proper connection limits

#### Metrics Tests
- `test_cosine_distance`: Validates distance calculations
- `test_cosine_similarity`: Tests similarity score computations
- `test_empty_vectors`: Ensures proper handling of empty vectors
- `test_zero_vectors`: Validates behavior with zero vectors

### 3. Memory Tests
Located in `tests/integration/temporal_test.rs`

#### Temporal Operations
- `test_temporal_ordering`: Validates temporal-based retrieval
- `test_temporal_attributes`: Tests timestamp and importance handling
- `test_temporal_decay`: Verifies decay calculations over time

#### Memory Storage
- `test_memory_storage_basic`: Tests basic storage operations
- `test_memory_storage_temporal`: Validates temporal aspects
- `test_memory_storage_importance`: Tests importance-based retrieval
- `test_memory_storage_concurrent`: Ensures thread safety

#### Relationship Handling
- `test_vector_relationships`: Tests relationship creation/retrieval
- `test_relationship_tracking`: Validates relationship maintenance
- `test_memory_consolidation`: Tests memory merging operations

#### Vector Operations
- `test_vector_dimensions`: Validates dimension handling
- `test_basic_vector_operations`: Tests vector manipulations

### 4. Context and Error Handling
- `test_context_operations`: Validates context management
- `test_error_handling`: Ensures proper error propagation

## 🔍 Test Coverage

Our test suite aims for comprehensive coverage across:
- Core functionality
- Edge cases
- Concurrent operations
- Error conditions
- Performance characteristics

## 🚀 Running Tests

Run the entire test suite:
```bash
cargo test
```

Run specific test categories:
```bash
cargo test temporal    # Run temporal-related tests
cargo test hnsw       # Run HNSW-related tests
cargo test storage    # Run storage-related tests
```

Run tests with logging:
```bash
RUST_LOG=debug cargo test
```

## 📊 Test Performance

Critical test performance metrics:
- Basic operations: < 1ms
- Concurrent operations: < 10ms
- Large-scale operations: < 100ms

## 🔄 Test Dependencies

Key test dependencies:
- `tokio`: Async runtime for concurrent testing
- `proptest`: Property-based testing
- `criterion`: Benchmarking

## 🛠️ Adding New Tests

When adding new tests:
1. Follow existing naming conventions
2. Include both positive and negative test cases
3. Document test purpose and expectations
4. Ensure proper error handling
5. Consider concurrent scenarios
6. Add relevant benchmarks

## 📝 Test Maintenance

Regular test maintenance includes:
1. Updating test vectors and expected results
2. Adjusting temporal parameters
3. Reviewing performance thresholds
4. Updating test documentation

## 🎯 Future Test Improvements

Planned improvements:
1. Enhanced property-based testing
2. Additional concurrent scenarios
3. Extended performance benchmarks
4. Improved temporal decay testing