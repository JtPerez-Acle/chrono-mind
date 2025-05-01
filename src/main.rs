use std::sync::Arc;
use std::path::PathBuf;
use std::fs;
use std::time::SystemTime;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use indicatif::{ProgressBar, ProgressStyle};
use vector_store::{
    storage::metrics::CosineDistance,
    memory::temporal::MemoryStorage,
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    core::config::MemoryConfig,
    storage::persistence::{StorageBackend, MemoryBackend},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[clap(name = "ChronoMind Vector Store")]
#[clap(author = "Vector Store Team")]
#[clap(version = VERSION)]
#[clap(about = "A temporal vector storage solution for AI applications", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Save vectors to a file
    Save {
        /// Input file containing vectors (JSON)
        #[clap(short, long)]
        input: String,

        /// Output file to save vectors
        #[clap(short, long)]
        output: String,

        /// Vector dimensions
        #[clap(short, long, default_value = "768")]
        dimensions: usize,

        /// Maximum number of memories
        #[clap(short, long, default_value = "1000")]
        max_memories: usize,

        /// Normalize vectors (convert to unit vectors)
        #[clap(short, long)]
        normalize: bool,
    },

    /// Query vectors from a file
    Query {
        /// File containing saved vectors
        #[clap(short, long)]
        file: String,

        /// Query vector (JSON array format)
        #[clap(short, long)]
        vector: String,

        /// Number of results to return
        #[clap(short, long, default_value = "10")]
        limit: usize,

        /// Filter by context
        #[clap(short, long)]
        context: Option<String>,

        /// Normalize query vector (convert to unit vector)
        #[clap(short, long)]
        normalize: bool,
    },

    /// Get statistics about stored vectors
    Stats {
        /// File containing saved vectors
        #[clap(short, long)]
        file: String,
    },
}

#[derive(Serialize, Deserialize)]
struct VectorInput {
    id: String,
    data: Vec<f32>,
    importance: Option<f32>,
    context: Option<String>,
    timestamp: Option<String>,
    decay_rate: Option<f32>,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Process commands
    let result = match cli.command {
        Commands::Save { input, output, dimensions, max_memories, normalize } => {
            save_vectors(input, output, dimensions, max_memories, normalize).await
        },
        Commands::Query { file, vector, limit, context, normalize } => {
            query_vectors(file, vector, limit, context, normalize).await
        },
        Commands::Stats { file } => {
            show_stats(file).await
        },
    };

    // Handle errors with user-friendly messages
    if let Err(err) = result {
        eprintln!("Error: {}", err);

        // Provide more specific guidance based on error type
        if err.to_string().contains("No such file or directory") {
            eprintln!("The specified file could not be found. Please check the path and try again.");
        } else if err.to_string().contains("InvalidVectorData") {
            eprintln!("The vector data is invalid. Please ensure it contains valid floating-point numbers.");
        } else if err.to_string().contains("InvalidDimensions") {
            eprintln!("The vector dimensions do not match the expected dimensions. Please check your configuration.");
        } else if err.to_string().contains("permission denied") {
            eprintln!("Permission denied when accessing the file. Please check your file permissions.");
        } else if err.to_string().contains("Invalid JSON") {
            eprintln!("The input file contains invalid JSON. Please check the file format.");
        }

        std::process::exit(1);
    }
}

async fn save_vectors(input: String, output: String, dimensions: usize, max_memories: usize, normalize: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Saving vectors to {}", output);
    if normalize {
        println!("Vector normalization enabled");
    }

    // Create configuration
    let config = MemoryConfig {
        max_dimensions: dimensions,
        max_memories,
        ..MemoryConfig::default()
    };

    // Create storage backend
    let mut backend = MemoryBackend::new(config);
    backend.init().await?;

    // Read input file
    let input_data = fs::read_to_string(input)?;
    let input_json: Value = serde_json::from_str(&input_data)?;

    // Process input data
    match input_json {
        // Single vector
        Value::Object(_) => {
            let mut vector_input: VectorInput = serde_json::from_str(&input_data)?;
            if normalize {
                normalize_vector(&mut vector_input.data);
            }
            save_single_vector(&mut backend, vector_input).await?;
        },
        // Array of vectors
        Value::Array(vectors) => {
            let total_vectors = vectors.len();
            let pb = ProgressBar::new(total_vectors as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} vectors ({eta})")
                .unwrap()
                .progress_chars("#>-"));

            for (i, vector_value) in vectors.iter().enumerate() {
                let vector_str = vector_value.to_string();
                let mut vector_input: VectorInput = serde_json::from_str(&vector_str)?;
                if normalize {
                    normalize_vector(&mut vector_input.data);
                }
                save_single_vector(&mut backend, vector_input).await?;
                pb.set_position((i + 1) as u64);
            }

            pb.finish_with_message("All vectors saved successfully");
        },
        _ => return Err("Invalid input format. Expected JSON object or array.".into()),
    }

    // Save to file
    let output_path = PathBuf::from(output);
    backend.backup(output_path).await?;

    println!("Vectors saved successfully");
    Ok(())
}

async fn save_single_vector(backend: &mut MemoryBackend, input: VectorInput) -> Result<(), Box<dyn std::error::Error>> {
    // Create vector
    let vector = Vector::new(
        input.id,
        input.data,
    );

    // Create temporal vector
    let temporal = TemporalVector::new(
        vector,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: input.importance.unwrap_or(0.5),
            context: input.context.unwrap_or_else(|| "default".to_string()),
            decay_rate: input.decay_rate.unwrap_or(0.1),
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );

    // Save to backend
    backend.save(&temporal).await?;

    Ok(())
}

async fn query_vectors(file: String, vector_str: String, limit: usize, context: Option<String>, normalize: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Querying vectors from {}", file);

    // Parse query vector
    let mut vector_data: Vec<f32> = parse_vector_string(&vector_str)?;

    // Normalize the query vector if requested
    if normalize {
        println!("Normalizing query vector");
        normalize_vector(&mut vector_data);
    }

    // Load from file
    let file_path = PathBuf::from(file);
    let config = MemoryConfig::default();
    let mut backend = MemoryBackend::new(config);
    backend.restore(file_path).await?;

    // Create memory storage for search
    let metric = Arc::new(CosineDistance::new());
    let mut storage = MemoryStorage::new(backend.get_config().clone(), metric);

    // Load memories from backend
    let memories = backend.list_all().await?;
    let total_memories = memories.len();

    let pb = ProgressBar::new(total_memories as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] Loading {pos}/{len} memories ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    for (i, memory) in memories.iter().enumerate() {
        storage.save_memory(memory.clone()).await?;
        pb.set_position((i + 1) as u64);
    }

    pb.finish_with_message("All memories loaded successfully");

    // Perform search
    let results = if let Some(ctx) = context {
        storage.search_by_context(&ctx, &vector_data, limit).await?
    } else {
        storage.search_similar(&vector_data, limit).await?
    };

    // Display results
    println!("Found {} results:", results.len());
    for (i, (memory, score)) in results.iter().enumerate() {
        println!("{}. ID: {}, Score: {:.4}, Importance: {:.2}, Context: {}",
            i + 1,
            memory.vector.id,
            score,
            memory.attributes.importance,
            memory.attributes.context
        );
    }

    Ok(())
}

async fn show_stats(file: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Showing statistics for {}", file);

    // Load from file
    let file_path = PathBuf::from(file);
    let config = MemoryConfig::default();
    let mut backend = MemoryBackend::new(config);
    backend.restore(file_path).await?;

    // Get stats
    let stats = backend.get_stats().await?;

    // Display stats
    println!("Total memories: {}", stats.total_memories);
    println!("Total size: {} bytes", stats.total_size);
    println!("Average vector size: {:.2} dimensions", stats.avg_vector_size);
    println!("Average importance: {:.4}", stats.average_importance);

    println!("\nContext distribution:");
    for (context, count) in stats.context_distribution {
        println!("  {}: {}", context, count);
    }

    println!("\nMost connected memories:");
    for (i, id) in stats.most_connected_memories.iter().enumerate() {
        println!("  {}. {}", i + 1, id);
    }

    Ok(())
}

fn parse_vector_string(vector_str: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // Handle JSON array format: [0.1, 0.2, 0.3]
    if vector_str.starts_with('[') && vector_str.ends_with(']') {
        let vector: Vec<f32> = serde_json::from_str(vector_str)?;
        return Ok(vector);
    }

    // Handle comma-separated format: 0.1,0.2,0.3
    let vector: Result<Vec<f32>, _> = vector_str
        .split(',')
        .map(|s| s.trim().parse::<f32>())
        .collect();

    Ok(vector?)
}

/// Normalize a vector to unit length (L2 norm)
fn normalize_vector(vector: &mut Vec<f32>) {
    // Calculate the L2 norm (Euclidean length) of the vector
    let norm: f32 = vector.iter().map(|&x| x * x).sum::<f32>().sqrt();

    // Avoid division by zero
    if norm > 0.0 {
        // Divide each component by the norm to get a unit vector
        for component in vector.iter_mut() {
            *component /= norm;
        }
    }
}
