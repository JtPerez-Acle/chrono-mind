use criterion::measurement::WallTime;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
    time::SystemTime,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkResult {
    timestamp: SystemTime,
    git_commit: String,
    rust_version: String,
    cpu_info: String,
    results: HashMap<String, GroupResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupResult {
    name: String,
    metrics: Vec<MetricResult>,
    comparison: Option<ComparisonResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricResult {
    name: String,
    value: f64,
    unit: String,
    batch_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResult {
    baseline_date: SystemTime,
    improvement: f64,
    regression: f64,
}

pub fn save_benchmark_results(results: BenchmarkResult) -> std::io::Result<()> {
    let date = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("benchmark_results_{}.json", date);
    let path = Path::new("benches/results").join(&filename);
    
    let json = serde_json::to_string_pretty(&results)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    
    generate_report(&results)?;
    Ok(())
}

fn generate_report(results: &BenchmarkResult) -> std::io::Result<()> {
    let mut report = String::new();
    report.push_str("# Benchmark Results Report\n\n");
    report.push_str(&format!("Date: {:?}\n", results.timestamp));
    report.push_str(&format!("Git Commit: {}\n", results.git_commit));
    report.push_str(&format!("Rust Version: {}\n", results.rust_version));
    report.push_str(&format!("CPU Info: {}\n\n", results.cpu_info));
    
    for (group_name, group) in &results.results {
        report.push_str(&format!("## {}\n\n", group_name));
        report.push_str("| Metric | Value | Unit | Batch Size |\n");
        report.push_str("|--------|-------|------|------------|\n");
        
        for metric in &group.metrics {
            report.push_str(&format!(
                "| {} | {:.2} | {} | {} |\n",
                metric.name, metric.value, metric.unit, metric.batch_size
            ));
        }
        
        if let Some(comparison) = &group.comparison {
            report.push_str("\n### Performance Changes\n");
            report.push_str(&format!(
                "- Improvement: {:.2}%\n",
                comparison.improvement * 100.0
            ));
            report.push_str(&format!(
                "- Regression: {:.2}%\n",
                comparison.regression * 100.0
            ));
        }
        
        report.push_str("\n");
    }
    
    let date = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("benchmark_report_{}.md", date);
    let path = Path::new("benches/results/analysis").join(&filename);
    
    let mut file = File::create(path)?;
    file.write_all(report.as_bytes())?;
    
    Ok(())
}
