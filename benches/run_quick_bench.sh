#!/bin/bash

echo "Running quick benchmarks for ChronoMind..."
echo "This will test core operations with a small dataset to avoid crashes."

# Set environment variables to limit resource usage
export RUST_MIN_STACK=8388608  # 8MB stack size
export RUST_BACKTRACE=1        # Enable backtraces for debugging

# Run the quick benchmark
cargo bench --bench quick_bench

echo "Benchmark complete. Results are available in target/criterion/"
