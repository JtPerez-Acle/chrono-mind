#!/bin/bash

# Get system information
GIT_COMMIT=$(git rev-parse HEAD)
RUST_VERSION=$(rustc --version)
CPU_INFO=$(lscpu | grep "Model name" | cut -d ':' -f 2 | xargs)

# Create results directory if it doesn't exist
mkdir -p benches/results/{memory,temporal,hnsw,analysis}

# Run benchmarks with detailed output
BENCHMARK_DATE=$(date +%Y-%m-%d_%H-%M-%S)
echo "🚀 Starting benchmark suite..."
cargo bench --bench main \
    --message-format=json \
    | tee "benches/results/raw_output_${BENCHMARK_DATE}.json"

# Generate plots
echo "📊 Generating plots..."
cargo criterion --bench main --plotting-backend plotters

# Move generated plots to results directory
echo "📁 Organizing results..."
find target/criterion -name "*.svg" -exec cp {} benches/results/ \;

# Generate summary report
echo "📝 Generating summary report..."
cat > "benches/results/analysis/summary_${BENCHMARK_DATE}.md" << EOF
# Benchmark Summary

Date: $(date)
Git Commit: ${GIT_COMMIT}
Rust Version: ${RUST_VERSION}
CPU Info: ${CPU_INFO}

## Results Location
- Raw data: \`benches/results/raw_output_${BENCHMARK_DATE}.json\`
- Plots: \`benches/results/*.svg\`
- Detailed analysis: \`benches/results/analysis/\`

## Quick Links
- Memory Operations: \`benches/results/memory/\`
- Temporal Operations: \`benches/results/temporal/\`
- HNSW Operations: \`benches/results/hnsw/\`

## Notes
- All benchmarks run with AVX-512 optimizations
- Results include throughput, latency, and resource utilization metrics
- Comparison with previous runs available in the analysis directory
EOF

echo "✨ Benchmark suite completed!"
echo "📊 Results saved to benches/results/"
echo "📑 Summary report: benches/results/analysis/summary_${BENCHMARK_DATE}.md"
