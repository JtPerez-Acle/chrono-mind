//! ChronoMind CLI: save vectors into a snapshot, query it, and inspect stats.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use serde_json::Value;

use chronomind::{
    load_snapshot, save_snapshot, ChronoMind, Config, Error, Memory, MemoryAttributes, Vector,
};

#[derive(Parser)]
#[command(name = "chronomind", version, about = "A temporal vector store", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Import vectors from a JSON file into a snapshot
    Save {
        /// Input JSON file (a vector object or an array of them)
        #[arg(short, long)]
        input: PathBuf,

        /// Output snapshot file
        #[arg(short, long)]
        output: PathBuf,

        /// Vector dimensions
        #[arg(short, long, default_value_t = 768)]
        dimensions: usize,

        /// Maximum number of memories
        #[arg(short, long, default_value_t = 100_000)]
        max_memories: usize,

        /// Normalize vectors to unit length before storing
        #[arg(short, long)]
        normalize: bool,
    },

    /// Query a snapshot for the nearest memories
    Query {
        /// Snapshot file to query
        #[arg(short, long)]
        file: PathBuf,

        /// Query vector: JSON array ("[0.1, 0.2]") or comma-separated ("0.1,0.2")
        #[arg(short, long)]
        vector: String,

        /// Number of results to return
        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        /// Restrict results to one context label
        #[arg(short, long)]
        context: Option<String>,

        /// Normalize the query vector to unit length
        #[arg(short, long)]
        normalize: bool,
    },

    /// Show statistics for a snapshot
    Stats {
        /// Snapshot file to inspect
        #[arg(short, long)]
        file: PathBuf,
    },
}

/// One vector record in the JSON input format.
#[derive(Deserialize)]
struct VectorInput {
    id: String,
    data: Vec<f32>,
    importance: Option<f32>,
    context: Option<String>,
    decay_rate: Option<f32>,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Save {
            input,
            output,
            dimensions,
            max_memories,
            normalize,
        } => save_command(&input, &output, dimensions, max_memories, normalize),
        Commands::Query {
            file,
            vector,
            limit,
            context,
            normalize,
        } => query_command(&file, &vector, limit, context.as_deref(), normalize),
        Commands::Stats { file } => stats_command(&file),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            eprint_hint(&err);
            ExitCode::FAILURE
        }
    }
}

fn eprint_hint(err: &Error) {
    match err {
        Error::Io(_) => {
            eprintln!("Check that the file path exists and that you have permission to access it.")
        }
        Error::InvalidDimensions { got, expected } => eprintln!(
            "The vector has {got} dimensions but the store expects {expected}. \
             Pass --dimensions {got} when saving, or fix the input."
        ),
        Error::InvalidVector(_) => {
            eprintln!("Ensure every vector component is a finite floating-point number.")
        }
        Error::InvalidSnapshot(_) => eprintln!(
            "The file is not a ChronoMind snapshot (or was written by an \
             incompatible version)."
        ),
        _ => {}
    }
}

fn save_command(
    input: &Path,
    output: &Path,
    dimensions: usize,
    max_memories: usize,
    normalize: bool,
) -> chronomind::Result<()> {
    let config = Config {
        dimensions,
        max_memories,
        ..Config::default()
    };
    let mut store = ChronoMind::new(config)?;

    let raw = std::fs::read_to_string(input)?;
    let parsed: Value = serde_json::from_str(&raw)?;
    let records: Vec<VectorInput> = match parsed {
        Value::Object(_) => vec![serde_json::from_value(parsed)?],
        Value::Array(_) => serde_json::from_value(parsed)?,
        _ => {
            return Err(Error::InvalidVector(
                "input must be a JSON object or array of objects".into(),
            ))
        }
    };

    let bar = progress_bar(records.len() as u64, "importing");
    for record in records {
        let mut data = record.data;
        if normalize {
            normalize_vector(&mut data);
        }
        store.insert(Memory::new(
            Vector::new(record.id, data),
            MemoryAttributes {
                importance: record.importance.unwrap_or(0.5),
                context: record.context.unwrap_or_else(|| "default".into()),
                decay_rate: record.decay_rate.unwrap_or(0.0),
                ..MemoryAttributes::default()
            },
        ))?;
        bar.inc(1);
    }
    bar.finish_and_clear();

    save_snapshot(&store, output)?;
    println!("Saved {} memories to {}", store.len(), output.display());
    Ok(())
}

fn query_command(
    file: &Path,
    vector: &str,
    limit: usize,
    context: Option<&str>,
    normalize: bool,
) -> chronomind::Result<()> {
    let store = load_snapshot(file)?;

    let mut query = parse_vector(vector)?;
    if normalize {
        normalize_vector(&mut query);
    }

    let results = match context {
        Some(ctx) => store.search_in_context(ctx, &query, limit)?,
        None => store.search(&query, limit)?,
    };

    if results.is_empty() {
        println!("No results.");
        return Ok(());
    }
    println!("Found {} result(s):", results.len());
    for (rank, (memory, score)) in results.iter().enumerate() {
        println!(
            "{}. id: {}  score: {:.4}  importance: {:.2}  context: {}",
            rank + 1,
            memory.vector.id,
            score,
            memory.attributes.importance,
            memory.attributes.context,
        );
    }
    Ok(())
}

fn stats_command(file: &Path) -> chronomind::Result<()> {
    let store = load_snapshot(file)?;
    let stats = store.stats();

    println!("Total memories:     {}", stats.total_memories);
    println!("Total components:   {}", stats.total_components);
    println!("Capacity used:      {:.1}%", stats.capacity_used * 100.0);
    println!("Average importance: {:.4}", stats.average_importance);

    if !stats.context_distribution.is_empty() {
        println!("\nContext distribution:");
        let mut contexts: Vec<_> = stats.context_distribution.iter().collect();
        contexts.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (context, count) in contexts {
            println!("  {context}: {count}");
        }
    }

    if !stats.most_referenced.is_empty() {
        println!("\nMost referenced memories:");
        for (rank, (id, count)) in stats.most_referenced.iter().enumerate() {
            println!("  {}. {id} ({count} references)", rank + 1);
        }
    }
    Ok(())
}

fn parse_vector(input: &str) -> chronomind::Result<Vec<f32>> {
    let trimmed = input.trim();
    if trimmed.starts_with('[') {
        return Ok(serde_json::from_str(trimmed)?);
    }
    trimmed
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f32>()
                .map_err(|e| Error::InvalidVector(format!("bad component {part:?}: {e}")))
        })
        .collect()
}

fn normalize_vector(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for component in vector.iter_mut() {
            *component /= norm;
        }
    }
}

fn progress_bar(len: u64, message: &'static str) -> ProgressBar {
    let bar = ProgressBar::new(len);
    bar.set_style(
        ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .expect("static template is valid")
            .progress_chars("#>-"),
    );
    bar.set_message(message);
    bar
}
