use criterion::{criterion_group, criterion_main, Criterion};
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;
use std::time::SystemTime;
use indicatif::{ProgressBar, ProgressStyle};
use colored::*;

mod common;
mod memory;
mod temporal;
mod hnsw;

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Runtime::new().unwrap()
});

pub fn run_async<F, R>(future: F) -> R 
where
    F: std::future::Future<Output = R>,
{
    RUNTIME.block_on(future)
}

fn print_benchmark_header(name: &str) {
    println!("\n{}", "╔═══════════════════════════════════════════════════════════════╗".bright_blue());
    println!("║ {:<61} ║", format!("🚀 {}", name).bright_white());
    println!("{}", "╚═══════════════════════════════════════════════════════════════╝".bright_blue());
}

fn create_progress_bar(name: String, total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
            .expect("Failed to create progress bar template")
            .progress_chars("#>-")
    );
    pb.set_message(name);
    pb
}

pub fn run_benchmarks(c: &mut criterion::Criterion) {
    println!("{}", "\n🔬 Vector Store Benchmark Suite".bright_green().bold());
    println!("{}", "============================".bright_green());
    
    let start_time = SystemTime::now();
    
    // Memory Operations Benchmarks
    print_benchmark_header("Memory Operations");
    let pb = create_progress_bar("Running memory benchmarks".to_string(), 100);
    memory::bench_memory_operations(c);
    pb.finish_with_message("Memory benchmarks completed");
    
    // Temporal Operations Benchmarks
    print_benchmark_header("Temporal Operations");
    let pb = create_progress_bar("Running temporal benchmarks".to_string(), 100);
    temporal::bench_temporal_operations(c);
    pb.finish_with_message("Temporal benchmarks completed");
    
    // HNSW Operations Benchmarks
    print_benchmark_header("HNSW Operations");
    let pb = create_progress_bar("Running HNSW benchmarks".to_string(), 100);
    hnsw::bench_hnsw_operations(c);
    pb.finish_with_message("HNSW benchmarks completed");
    
    if let Ok(elapsed) = SystemTime::now().duration_since(start_time) {
        println!("\n{}", "╔═══════════════════════════════════════════════════════════════╗".bright_green());
        println!("║ {:<61} ║", "✨ Benchmark Summary".bright_white());
        println!("╟───────────────────────────────────────────────────────────────╢");
        println!("║ {:<61} ║", format!("Total Time: {:.2} seconds", elapsed.as_secs_f64()).bright_yellow());
        println!("║ {:<61} ║", "All benchmarks completed successfully".bright_green());
        println!("{}", "╚═══════════════════════════════════════════════════════════════╝".bright_green());
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10) // Reduced sample size for faster feedback
        .measurement_time(std::time::Duration::from_secs(5))
        .warm_up_time(std::time::Duration::from_secs(1));
    targets = run_benchmarks
}
criterion_main!(benches);
