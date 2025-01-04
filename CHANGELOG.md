# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- New modular codebase structure with clear separation of concerns:
  - `core`: Essential components (error, config, logging)
  - `memory`: Memory management (temporal, traits, types)
  - `storage`: Storage implementations and metrics
  - `utils`: Validation and monitoring utilities
- Temporal-aware similarity metrics with:
  - Time-based decay
  - Importance weighting
  - Access frequency consideration
- Comprehensive test suite with:
  - Property-based testing
  - Concurrent access testing
  - Memory validation
  - Context operations testing
- Memory system features:
  - Automatic memory decay
  - Importance-based cleanup
  - Context-based organization
  - Relationship tracking
  - Memory consolidation
  - Health monitoring
  - Performance metrics
- Error handling:
  - Custom error types with thiserror
  - Validation checks
  - Proper error propagation
- Configuration system:
  - Memory limits
  - Decay parameters
  - Context settings
  - Relationship limits
- Monitoring utilities:
  - Performance tracking
  - Health checks
  - Memory statistics
  - Context summaries
- Temporal-aware HNSW implementation:
  - Multi-layer graph structure
  - Temporal score integration
  - Efficient neighbor selection
  - Dynamic layer management
  - Concurrent access support
  - Configurable parameters

### Changed
- Restructured entire codebase for better modularity
- Improved code organization with clear module boundaries
- Enhanced public API with better type re-exports
- Optimized vector operations for better performance
- Improved error handling with specific error types
- Enhanced logging system with structured output
- Upgraded test suite with comprehensive coverage
- Refactored configuration system for better usability
- Improved memory management algorithms
- Enhanced similarity search with temporal awareness
- Optimized HNSW graph construction
- Improved neighbor selection algorithm

### Removed
- Legacy monolithic code structure
- Old error handling system
- Outdated logging implementation
- Legacy test suite that was built for basic CRUD operations
- Simple vector storage tests without temporal aspects
- Unsafe concurrent access patterns
- Ad-hoc test data generation
- Basic vector storage trait
- Unused utility functions
- Legacy main.rs implementation
- Basic similarity search implementation
- Memory-mapped storage implementation
- HNSW implementation (to be replaced with temporal-aware version)

### Fixed
- Memory leaks in concurrent operations
- Race conditions in memory updates
- Inconsistent error handling
- Configuration validation issues
- Memory cleanup timing issues
- Context switching bugs
- Relationship tracking errors
- Performance bottlenecks in vector operations
- Floating-point precision issues
- HNSW graph construction edge cases
- Neighbor selection stability
- Improved error handling in HNSW implementation:
  - Proper node access validation
  - Memory error propagation
- Enhanced temporal score calculation:
  - Use memory-specific decay rate
  - Improved importance weighting
- Improved memory decay algorithm:
  - Consider time since last access
  - Use memory-specific decay rates
  - Added recency factor to decay calculation
  - Better logging of decay factors
- Fixed HNSW search ranking:
  - Correct distance normalization
  - Proper handling of temporal weights
  - Fixed pure distance-based sorting

### In Progress
- Implementing temporal-aware HNSW for efficient similarity search
- Adding file-based persistence with backup/restore
- Creating comprehensive benchmarking suite
- Developing memory sharding capabilities
- Implementing advanced memory consolidation algorithms

## [0.1.0] - 2025-01-03
### Added
- Initial release with basic vector storage functionality
- Simple CRUD operations for vectors
- Basic similarity search capabilities
- Minimal error handling
- Basic test coverage

### Dependencies
- Tokio 1.35 for async runtime
- Serde 1.0 for serialization
- Memmap2 0.9 for memory mapping
- Tracing 0.1 for logging and diagnostics
