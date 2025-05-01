# ChronoMind Data Flow Analysis

This document provides a detailed analysis of how data flows through the ChronoMind system, from vector creation to storage, search, and temporal decay.

## Core Data Structures

### Vector

The basic unit of data in ChronoMind is a `Vector`, which contains:
- `id`: A unique identifier (String)
- `data`: The vector data (Vec<f32>)

### TemporalVector

Extends `Vector` with temporal attributes:
- `vector`: The base Vector
- `attributes`: MemoryAttributes containing temporal metadata

### MemoryAttributes

Contains temporal metadata for vectors:
- `timestamp`: When the vector was created
- `importance`: A float value representing importance (0.0-1.0)
- `context`: A string tag for grouping related vectors
- `decay_rate`: How quickly importance decays over time
- `relationships`: Links to other related vectors
- `access_count`: How many times the vector has been accessed
- `last_access`: When the vector was last accessed

## Data Flow Paths

### 1. Vector Insertion Flow

```
Client → Vector Creation → Normalization → TemporalVector Creation → MemoryStorage.save_memory()
                                                                       │
                                                                       ▼
                                                                     Memory Store
                                                                       │
                                                                       ▼
                                                                     HNSW Index
```

1. Client creates a vector with data and ID
2. Vector is normalized (optional but recommended)
3. TemporalVector is created with vector and attributes
4. MemoryStorage.save_memory() is called
5. Vector is stored in the memory store
6. Vector is indexed in the HNSW graph for fast retrieval

### 2. Vector Search Flow

```
Client → Query Vector → Normalization → MemoryStorage.search_similar()
                                           │
                                           ▼
                                        HNSW Index
                                           │
                                           ▼
                                     Approximate KNN
                                           │
                                           ▼
                                    Temporal Reranking
                                           │
                                           ▼
                                     Results to Client
```

1. Client provides a query vector
2. Vector is normalized
3. MemoryStorage.search_similar() is called
4. HNSW index performs approximate k-nearest neighbors search
5. Results are reranked based on temporal factors (importance, recency)
6. Final results are returned to the client

### 3. Memory Decay Flow

```
Periodic Trigger → MemoryStorage.update_memory_decay()
                      │
                      ▼
                  Iterate Memories
                      │
                      ▼
                Apply Decay Formula
                      │
                      ▼
                Update Importance
```

1. Periodic trigger (e.g., scheduled task) initiates decay
2. MemoryStorage.update_memory_decay() is called
3. System iterates through all stored memories
4. Decay formula is applied based on time elapsed and decay_rate
5. Importance values are updated

### 4. Context-Based Filtering Flow

```
Client → Context + Query → MemoryStorage.search_by_context()
                              │
                              ▼
                         Filter by Context
                              │
                              ▼
                        Similarity Search
                              │
                              ▼
                       Results to Client
```

1. Client provides a context tag and query vector
2. MemoryStorage.search_by_context() is called
3. System filters vectors by the specified context
4. Similarity search is performed on the filtered set
5. Results are returned to the client

### 5. Relationship Traversal Flow

```
Client → Vector ID → MemoryStorage.get_related_memories()
                        │
                        ▼
                   Lookup Vector
                        │
                        ▼
                Extract Relationships
                        │
                        ▼
                  Fetch Related Vectors
                        │
                        ▼
                 Results to Client
```

1. Client provides a vector ID
2. MemoryStorage.get_related_memories() is called
3. System looks up the specified vector
4. Relationships are extracted from the vector's attributes
5. Related vectors are fetched
6. Results are returned to the client

## Data Transformation

### Vector Normalization

Vectors are normalized to unit length to ensure consistent similarity calculations:

```
normalized_vector = vector / ||vector||
```

Where `||vector||` is the L2 norm (Euclidean length) of the vector.

### Similarity Calculation

Similarity between vectors is calculated using cosine similarity:

```
similarity = dot_product(v1, v2) / (||v1|| * ||v2||)
```

For normalized vectors, this simplifies to:

```
similarity = dot_product(v1, v2)
```

### Temporal Reranking

Search results are reranked based on a combination of similarity and temporal factors:

```
final_score = similarity * (1 - temporal_weight) + temporal_score * temporal_weight
```

Where `temporal_score` is calculated from importance and recency:

```
temporal_score = importance * recency_factor
```

### Importance Decay

Importance decays over time according to:

```
new_importance = importance * exp(-decay_rate * time_elapsed)
```

## Data Persistence

The current implementation primarily uses in-memory storage with the following components:

1. **Memory Store**: HashMap-based storage for all vectors and their attributes
2. **HNSW Index**: Multi-layer graph structure for efficient similarity search
3. **Persistence Layer**: (Partially implemented) For saving and loading vectors to/from disk

## Concurrency Model

ChronoMind uses a concurrent access model with the following characteristics:

1. **Read-Write Locks**: `parking_lot::RwLock` for efficient concurrent access
2. **Lock Granularity**: Separate locks for the memory store and HNSW index
3. **Async Operations**: All public API methods are async for non-blocking I/O

## Conclusion

The data flow in ChronoMind is well-structured with clear paths for the main operations: insertion, search, decay, and relationship traversal. The system effectively combines vector similarity search with temporal awareness, allowing for more human-like memory retrieval that prioritizes important and recent memories.

The architecture supports concurrent access and provides good performance characteristics, with sub-millisecond operations even for large vectors. The modular design allows for future extensions, such as more sophisticated persistence mechanisms or distributed storage.
