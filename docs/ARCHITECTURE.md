# Vector Storage Architecture

## Directory Structure

```
rust/
├── src/
│   ├── error.rs           # Error types and implementations
│   ├── error/             # Additional error handling
│   ├── lib.rs            # Library entry point
│   ├── main.rs           # Binary entry point
│   ├── storage/
│   │   ├── data_dir.rs   # Data directory management
│   │   ├── memory.rs     # In-memory storage implementation
│   │   ├── metrics.rs    # Storage metrics
│   │   ├── mmap.rs       # Memory-mapped storage implementation
│   │   ├── mod.rs        # Storage traits and implementations
│   │   └── hnsw/         # HNSW index implementation
│   │       ├── mod.rs    # Main HNSW index type and implementation
│   │       ├── config.rs # HNSW configuration parameters
│   │       ├── node.rs   # Node structure for HNSW graph
│   │       ├── insert.rs # Vector insertion logic
│   │       ├── search.rs # Search implementation
│   │       └── candidate.rs # Candidate type for search queue
│   ├── utils.rs          # Utility functions
│   └── utils/            # Additional utilities
├── benches/
│   └── vector_ops.rs     # Performance benchmarks
├── tests/
│   ├── unit/            # Unit tests
│   │   └── memory_storage_test.rs  # Memory storage unit tests
│   └── integration/     # Integration tests
│       └── storage_test.rs  # Storage backend integration tests
└── docs/
    ├── ARCHITECTURE.md   # This file
    └── HNSW.md          # HNSW implementation details
```

## Proposed Modularization

To improve modularity, we should:

1. **Storage Layer Separation**
   - Move each storage implementation into its own module:
     ```
     storage/
     ├── memory/
     │   ├── mod.rs
     │   └── metrics.rs
     ├── mmap/
     │   ├── mod.rs
     │   └── format.rs
     └── hnsw/
         └── (current structure)
     ```

2. **Error Handling**
   - Consolidate error types into a single module
   - Remove redundant error directory

3. **Testing Structure**
   - Unit tests in `tests/unit/`
   - Integration tests in `tests/integration/`
   - Benchmarks in `benches/`
   - Property-based tests (to be added)

4. **Configuration**
   - Add dedicated config module for app-wide settings
   - Move HNSW config into general config structure

5. **Documentation**
   - Add API documentation
   - Add examples directory
   - Improve benchmarking documentation

## Components

### Storage Layer

The project implements multiple storage backends for vector data:

1. **Memory Storage** (memory.rs)
   - In-memory implementation using HashMap
   - Supports basic CRUD operations
   - Configurable distance metrics
   - Suitable for testing and small datasets

2. **Memory-Mapped Storage** (mmap.rs)
   - Persistent storage using memory-mapped files
   - Efficient disk-based storage with fast access
   - File format versioning
   - Supports large datasets

3. **Data Directory** (data_dir.rs)
   - Manages storage directory structure
   - Handles file organization and cleanup

4. **Metrics** (metrics.rs)
   - Distance metric implementations
   - Currently supports Euclidean distance
   - Extensible for additional metrics

### HNSW Index

The Hierarchical Navigable Small World (HNSW) index implementation:

1. **HnswIndex** (mod.rs)
   - Main index structure
   - Manages the graph of nodes
   - Coordinates insert and search operations

2. **Node** (node.rs)
   - Represents a vertex in the HNSW graph
   - Stores vector data and connections
   - Manages layer assignments

3. **Config** (config.rs)
   - Configuration parameters for HNSW
   - M: Maximum number of connections per node
   - ef_construction: Size of dynamic candidate list for construction
   - ef: Size of dynamic candidate list for search

4. **Insert** (insert.rs)
   - Handles vector insertion
   - Manages layer assignment
   - Creates and updates connections

5. **Search** (search.rs)
   - Implements nearest neighbor search
   - Uses priority queues for efficient candidate management
   - Supports layer-wise traversal

6. **Candidate** (candidate.rs)
   - Helper type for search queue management
   - Stores distance information for priority queue

## Design Decisions

1. **Modular Architecture**
   - Each component is in its own file for better organization
   - Clear separation of concerns
   - Easier testing and maintenance

2. **Error Handling**
   - Custom error types for better error reporting
   - Consistent error handling across components
   - Detailed error messages for debugging

3. **Logging**
   - Structured logging using tracing
   - Debug logs for development and troubleshooting
   - Performance metrics logging

4. **Testing**
   - Unit tests for each component
   - Integration tests for end-to-end validation
   - Property-based testing for robustness

## Future Improvements

1. **Performance Optimization**
   - SIMD operations for distance calculations
   - Parallel search implementation
   - Memory-mapped storage backend

2. **Persistence**
   - Serialization format for nodes and connections
   - Incremental updates
   - Backup and restore functionality

3. **Monitoring**
   - Performance metrics collection
   - Health checks
   - Resource usage monitoring
