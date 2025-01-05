# Vector Store Benchmark Expectations and Results

This document outlines the expected performance metrics for our vector store across different dimensions and scenarios. Each metric includes three tiers of performance targets: Excellent, Good, and Decent. Additionally, it includes the current benchmark results.

## Core Performance Metrics

### 1. Search Latency (P99)
Single vector search against 1M vectors:

| Performance Tier | BERT (768d) | Ada-002 (1536d) | MiniLM (384d) |
|-----------------|-------------|-----------------|---------------|
| Excellent | < 100ns | < 150ns | < 50ns |
| Good | < 200ns | < 300ns | < 100ns |
| Decent | < 500ns | < 750ns | < 250ns |

*Measurement Method*: Average over 10,000 queries with 100 concurrent users

### 2. Throughput (QPS)
Queries per second with 100 concurrent users:

| Performance Tier | Small (10K) | Medium (100K) | Large (1M) |
|-----------------|-------------|---------------|------------|
| Excellent | > 10M | > 5M | > 1M |
| Good | > 5M | > 2M | > 500K |
| Decent | > 2M | > 1M | > 200K |

*Measurement Method*: Sustained throughput over 5 minutes

### 3. Memory Efficiency
RAM usage per vector:

| Performance Tier | BERT (768d) | Ada-002 (1536d) | MiniLM (384d) |
|-----------------|-------------|-----------------|---------------|
| Excellent | < 3KB | < 6KB | < 1.5KB |
| Good | < 4KB | < 8KB | < 2KB |
| Decent | < 6KB | < 12KB | < 3KB |

*Measurement Method*: Total RAM / Number of vectors after 1M insertions

## HNSW-Specific Metrics

### 4. Index Build Time
Time to build index for 1M vectors:

| Performance Tier | Fast Profile | Balanced Profile | Quality Profile |
|-----------------|--------------|------------------|-----------------|
| Excellent | < 10min | < 20min | < 40min |
| Good | < 20min | < 40min | < 80min |
| Decent | < 40min | < 80min | < 160min |

*Measurement Method*: Single-threaded build time, average of 3 runs

### 5. Search Accuracy (Recall@10)

| Performance Tier | Fast Profile | Balanced Profile | Quality Profile |
|-----------------|--------------|------------------|-----------------|
| Excellent | > 0.90 | > 0.95 | > 0.98 |
| Good | > 0.85 | > 0.90 | > 0.95 |
| Decent | > 0.80 | > 0.85 | > 0.90 |

*Measurement Method*: Ground truth comparison with exact search, 10K queries

## Temporal Features

### 6. Temporal Query Latency
Search with temporal decay and importance weighting:

| Performance Tier | Simple Decay | With Importance | Full Context |
|-----------------|--------------|-----------------|--------------|
| Excellent | < 150ns | < 200ns | < 300ns |
| Good | < 300ns | < 400ns | < 600ns |
| Decent | < 600ns | < 800ns | < 1200ns |

*Measurement Method*: P99 latency over 10K queries

### 7. Memory Decay Performance
Time to update temporal scores for 1M vectors:

| Performance Tier | Daily Update | Hourly Update | Real-time |
|-----------------|--------------|---------------|-----------|
| Excellent | < 1s | < 100ms | < 10ms |
| Good | < 2s | < 200ms | < 20ms |
| Decent | < 5s | < 500ms | < 50ms |

*Measurement Method*: Average time over 100 updates

## Resource Utilization

### 8. CPU Usage
Percentage during high load (100 concurrent users):

| Performance Tier | Search Only | With Updates | Full Load |
|-----------------|-------------|--------------|-----------|
| Excellent | < 30% | < 50% | < 70% |
| Good | < 40% | < 60% | < 80% |
| Decent | < 50% | < 70% | < 90% |

*Measurement Method*: Average over 5 minutes on 8-core CPU

### 9. Index Size Overhead
Additional space required by HNSW graph:

| Performance Tier | Fast Profile | Balanced Profile | Quality Profile |
|-----------------|--------------|------------------|-----------------|
| Excellent | < 10% | < 20% | < 30% |
| Good | < 15% | < 25% | < 35% |
| Decent | < 20% | < 30% | < 40% |

*Measurement Method*: (Total size - Raw vector size) / Raw vector size

## Test Scenarios

### 10. Mixed Workload Performance
Combined read/write operations:

| Performance Tier | Light Load | Medium Load | Heavy Load |
|-----------------|------------|-------------|------------|
| Excellent | < 150ns | < 300ns | < 600ns |
| Good | < 300ns | < 600ns | < 1.2µs |
| Decent | < 600ns | < 1.2µs | < 2.4µs |

*Test Configuration*:
- Light: 80% read, 20% write, 10 concurrent users
- Medium: 70% read, 30% write, 50 concurrent users
- Heavy: 60% read, 40% write, 100 concurrent users

## Current Benchmark Results

### Completed Tests

#### Small Dataset (10K vectors)
- Chat History:
  - ExactMatch: 69.8µs (slight regression +9.6%)
  - Semantic: 1.19ms (stable)
  - Hybrid: 520.9µs (improved -11.8%)

- Knowledge Base:
  - ExactMatch: 59.6µs (improved -22.7%)
  - Semantic: 760.5µs (improved -12.9%)
  - Hybrid: 397.5µs (improved -17.2%)

- Mixed Workload:
  - ExactMatch: 105.1µs (regression +45.6%)
  - Semantic: 798.5µs (improved -9.2%)
  - Hybrid: 437.5µs (stable)

#### Medium Dataset (100K vectors)
- Chat History:
  - ExactMatch: 279.2µs (improved -32.1%)
  - Semantic: 3.50ms (improved -5.5%)
  - Hybrid: 1.79ms (improved -6.4%)

- Knowledge Base:
  - ExactMatch: 443.8µs (regression +9.9%)
  - Semantic: 5.28ms (stable)
  - Hybrid: 2.70ms (stable)

- Mixed Workload:
  - ExactMatch: 660.1µs (improved -11.7%)
  - Semantic: 5.69ms (stable)
  - Hybrid: 2.85ms (improved -4.3%)

### Performance Analysis

#### Strengths
1. **ExactMatch Queries**: Excellent performance on small datasets (59-105µs)
2. **Consistent Improvements**: Most operations showed performance improvements
3. **Hybrid Search**: Good balance of speed and accuracy

#### Areas for Investigation
1. **Mixed Workload ExactMatch**: Significant regression (+45.6%) in small dataset
2. **Knowledge Base ExactMatch**: Regression (+9.9%) in medium dataset
3. **Outliers**: Several tests showed high outliers, indicating potential stability issues

### Crash Analysis
The benchmark crashed while scaling up to larger datasets (at ~400K vectors). Possible causes:
1. Memory exhaustion
2. Vector allocation issues
3. Resource limits

### Next Steps

1. **Immediate Actions**
   - Implement memory monitoring in benchmark suite
   - Add graceful degradation for large datasets
   - Investigate ExactMatch regressions

2. **Future Benchmarks**
   - Complete BERT-base large dataset tests
   - Run Ada-002 and MiniLM model tests
   - Add memory usage tracking

3. **Optimization Targets**
   - Improve mixed workload stability
   - Reduce outliers in measurements
   - Optimize memory usage for large datasets

## Performance vs. Expectations

| Metric | Target | Current | Status |
|--------|---------|---------|---------|
| Small Dataset Latency | < 100µs | 59.6-798.5µs | ⚠️ Mixed |
| Medium Dataset Latency | < 1ms | 279.2µs-5.69ms | ⚠️ Mixed |
| Memory Efficiency | < 4KB/vector | Unknown | 📝 Need Data |
| Stability | < 10% outliers | 10-30% outliers | ❌ Need Work |

## Recommendations

1. **Memory Management**
   - Implement incremental vector loading
   - Add memory usage monitoring
   - Consider vector compression

2. **Stability Improvements**
   - Add warm-up cycles before measurements
   - Implement automatic outlier detection
   - Add resource monitoring

3. **Measurement Refinements**
   - Add detailed memory tracking
   - Implement progressive load testing
   - Add system resource monitoring

## Measurement Guidelines

1. **Warm-up Period**
   - 30 seconds minimum before measurements
   - Ensure CPU temperature is stable
   - Clear OS page cache between runs

2. **Sample Sizes**
   - Latency: 10,000 queries minimum
   - Throughput: 5-minute sustained test
   - Build time: Average of 3 runs
   - Memory: Measured after GC cycle

3. **Environment Requirements**
   - CPU: 8 cores minimum
   - RAM: 32GB minimum
   - Storage: NVMe SSD
   - OS: Linux with kernel 5.10+

4. **Test Data**
   - Real-world text embeddings
   - Varied content length
   - Natural temporal distribution
   - Mixed importance levels

## Reporting Format

Results should be reported in JSON format:
```json
{
  "test_name": "search_latency",
  "configuration": {
    "profile": "balanced",
    "model": "bert-base",
    "dataset_size": 1000000
  },
  "results": {
    "p50": "84.93ns",
    "p99": "98.72ns",
    "qps": 10000000,
    "memory_usage": "3KB/vector",
    "accuracy": 0.95
  },
  "environment": {
    "cpu": "AMD Ryzen 9 5950X",
    "ram": "64GB",
    "os": "Ubuntu 22.04"
  }
}
```

## Success Criteria

A benchmark run is considered successful when:
1. All metrics meet at least "Decent" tier
2. No individual test exceeds 2x the "Decent" threshold
3. Results are reproducible within ±5%
4. No resource exhaustion during tests
5. All data consistency checks pass