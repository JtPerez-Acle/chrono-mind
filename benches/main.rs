#![cfg(test)]
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ndarray::{Array1, Array2};
use num_complex::Complex64;
use rand::prelude::*;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::time::Duration;

const NEURAL_DIMS: usize = 256;
const QUANTUM_DIMS: usize = 256;
const COMPRESSION_RATIOS: [f64; 4] = [0.25, 0.50, 0.75, 1.0];
const BATCH_SIZES: [usize; 4] = [1, 10, 100, 1000];

#[derive(Debug)]
struct NeuralCompressor {
    weights: Array2<f64>,
    bias: Array1<f64>,
}

impl NeuralCompressor {
    fn new(input_dims: usize, compression_ratio: f64, rng: &mut impl RngCore) -> Self {
        let output_dims = (input_dims as f64 * compression_ratio) as usize;
        
        // Initialize weights with He initialization
        let scale = (2.0 / input_dims as f64).sqrt();
        let weights = Array2::from_shape_fn((output_dims, input_dims), |_| {
            rng.gen::<f64>() * 2.0 * scale - scale
        });
        
        let bias = Array1::from_shape_fn(output_dims, |_| {
            rng.gen::<f64>() * 0.1
        });
        
        Self {
            weights,
            bias,
        }
    }
    
    fn compress(&self, input: &Array1<f64>) -> Array1<f64> {
        let mut output = self.weights.dot(input);
        output += &self.bias;
        output.mapv_inplace(|x| x.max(0.0)); // ReLU activation
        output
    }
}

#[derive(Debug)]
struct TemporalFusion {
    weights: Array2<f64>,
    bias: Array1<f64>,
    temporal_scale: f64,
}

impl TemporalFusion {
    fn new(dims: usize, rng: &mut impl RngCore) -> Self {
        let weights = Array2::from_shape_fn((dims, dims), |_| {
            rng.gen::<f64>() * 2.0 - 1.0
        });
        
        let bias = Array1::from_shape_fn(dims, |_| {
            rng.gen::<f64>() * 0.1
        });
        
        Self {
            weights,
            bias,
            temporal_scale: 0.1,
        }
    }
    
    fn fuse_vectors(&self, vectors: &[Array1<f64>]) -> Array1<f64> {
        if vectors.is_empty() {
            return Array1::zeros(self.weights.nrows());
        }
        
        let mut result = vectors[0].clone();
        for v in vectors.iter().skip(1) {
            let mut temp = self.weights.dot(v);
            temp += &self.bias;
            temp *= self.temporal_scale;
            result += &temp;
        }
        
        result /= vectors.len() as f64;
        result
    }
}

#[derive(Debug, Clone)]
struct QuantumState {
    amplitudes: Vec<Complex64>,
}

impl QuantumState {
    fn new(dims: usize, rng: &mut impl RngCore) -> Self {
        let mut amplitudes = Vec::with_capacity(dims);
        let mut sum_sq = 0.0;
        
        // Generate random complex amplitudes
        for _ in 0..dims {
            let re = rng.gen::<f64>() * 2.0 - 1.0;
            let im = rng.gen::<f64>() * 2.0 - 1.0;
            let amp = Complex64::new(re, im);
            amplitudes.push(amp);
            sum_sq += amp.norm_sqr();
        }
        
        // Normalize
        let norm = sum_sq.sqrt();
        for amp in &mut amplitudes {
            *amp /= norm;
        }
        
        Self { amplitudes }
    }
    
    fn apply_operator(&mut self, operator: &[Vec<Complex64>]) {
        let mut new_amplitudes = vec![Complex64::new(0.0, 0.0); self.amplitudes.len()];
        
        for i in 0..self.amplitudes.len() {
            for j in 0..self.amplitudes.len() {
                new_amplitudes[i] += operator[i][j] * self.amplitudes[j];
            }
        }
        
        self.amplitudes = new_amplitudes;
    }
    
    fn measure(&self) -> usize {
        let mut rng = thread_rng();
        let mut cumsum = 0.0;
        let r = rng.gen::<f64>();
        
        for (i, amp) in self.amplitudes.iter().enumerate() {
            cumsum += amp.norm_sqr();
            if r <= cumsum {
                return i;
            }
        }
        
        self.amplitudes.len() - 1
    }
}

pub fn all_benchmarks(c: &mut Criterion) {
    // Neural benchmarks
    bench_neural_compression(c);
    bench_temporal_fusion(c);
    bench_adaptive_precision(c);
    
    // Quantum benchmarks
    bench_quantum_search(c);
    bench_quantum_coherence(c);
    bench_quantum_entanglement(c);
}

pub fn bench_neural_compression(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let mut group = c.benchmark_group("neural_compression");
    group.measurement_time(Duration::from_secs(10));
    
    for &ratio in &COMPRESSION_RATIOS {
        group.bench_with_input(format!("ratio_{}", ratio), &ratio, |b, &ratio| {
            let compressor = NeuralCompressor::new(NEURAL_DIMS, ratio, &mut rng);
            let input = Array1::from_shape_fn(NEURAL_DIMS, |_| rng.gen::<f64>());
            
            b.iter(|| {
                black_box(compressor.compress(&input));
            });
        });
    }
    
    group.finish();
}

pub fn bench_temporal_fusion(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let mut group = c.benchmark_group("temporal_fusion");
    group.measurement_time(Duration::from_secs(10));
    
    for &size in &[2, 4, 8, 16] {
        group.bench_with_input(format!("vectors_{}", size), &size, |b, &size| {
            let fusion = TemporalFusion::new(NEURAL_DIMS, &mut rng);
            let vectors: Vec<_> = (0..size)
                .map(|_| Array1::from_shape_fn(NEURAL_DIMS, |_| rng.gen::<f64>()))
                .collect();
            
            b.iter(|| {
                black_box(fusion.fuse_vectors(&vectors));
            });
        });
    }
    
    group.finish();
}

pub fn bench_adaptive_precision(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let mut group = c.benchmark_group("adaptive_precision");
    group.measurement_time(Duration::from_secs(10));
    
    for precision in [16, 32, 64] {
        group.bench_with_input(format!("bits_{}", precision), &precision, |b, _| {
            let input = Array1::from_shape_fn(NEURAL_DIMS, |_| rng.gen::<f64>());
            let scale = 2.0f64.powi(precision);
            
            b.iter(|| {
                let mut quantized = input.clone();
                quantized.mapv_inplace(|x| (x * scale).round() / scale);
                black_box(quantized);
            });
        });
    }
    
    group.finish();
}

pub fn bench_quantum_search(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let mut group = c.benchmark_group("quantum_search");
    group.measurement_time(Duration::from_secs(10));
    
    for &batch_size in &BATCH_SIZES {
        group.bench_with_input(format!("batch_{}", batch_size), &batch_size, |b, &size| {
            let state = QuantumState::new(QUANTUM_DIMS, &mut rng);
            let states: Vec<_> = (0..size).map(|_| state.clone()).collect();
            
            b.iter(|| {
                black_box(states.iter().map(|s| s.measure()).collect::<Vec<_>>());
            });
        });
    }
    
    group.finish();
}

pub fn bench_quantum_coherence(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let mut group = c.benchmark_group("quantum_coherence");
    group.measurement_time(Duration::from_secs(10));
    
    let hadamard: Vec<Vec<Complex64>> = (0..QUANTUM_DIMS)
        .map(|i| {
            (0..QUANTUM_DIMS)
                .map(|j| {
                    let phase = if (i & j).count_ones() % 2 == 0 { 1.0 } else { -1.0 };
                    Complex64::new(phase / (QUANTUM_DIMS as f64).sqrt(), 0.0)
                })
                .collect()
        })
        .collect();
    
    for &batch_size in &BATCH_SIZES {
        group.bench_with_input(format!("batch_{}", batch_size), &batch_size, |b, &size| {
            let mut states: Vec<_> = (0..size)
                .map(|_| QuantumState::new(QUANTUM_DIMS, &mut rng))
                .collect();
            
            b.iter(|| {
                for state in &mut states {
                    state.apply_operator(&hadamard);
                }
                black_box(&states);
            });
        });
    }
    
    group.finish();
}

pub fn bench_quantum_entanglement(c: &mut Criterion) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(42);
    let mut group = c.benchmark_group("quantum_entanglement");
    group.measurement_time(Duration::from_secs(10));
    
    // CNOT gate
    let cnot: Vec<Vec<Complex64>> = (0..QUANTUM_DIMS)
        .map(|i| {
            (0..QUANTUM_DIMS)
                .map(|j| {
                    if i == j {
                        Complex64::new(1.0, 0.0)
                    } else if (i ^ 1) == j && (i & 1) == 1 {
                        Complex64::new(1.0, 0.0)
                    } else {
                        Complex64::new(0.0, 0.0)
                    }
                })
                .collect()
        })
        .collect();
    
    for &batch_size in &BATCH_SIZES {
        group.bench_with_input(format!("batch_{}", batch_size), &batch_size, |b, &size| {
            let mut states: Vec<_> = (0..size)
                .map(|_| QuantumState::new(QUANTUM_DIMS, &mut rng))
                .collect();
            
            b.iter(|| {
                for state in &mut states {
                    state.apply_operator(&cnot);
                }
                black_box(&states);
            });
        });
    }
    
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(10));
    targets = bench_neural_compression, bench_temporal_fusion, bench_adaptive_precision,
             bench_quantum_search, bench_quantum_coherence, bench_quantum_entanglement
}

criterion_main!(benches);
