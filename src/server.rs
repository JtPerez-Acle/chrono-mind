use std::time::SystemTime;
use crate::{
    config::Config,
    core::error::Result,
    memory::{
        traits::VectorStorage,
        types::{MemoryAttributes, TemporalVector, Vector},
    },
    storage::persistence::MemoryBackend,
};

pub struct Server {
    config: Config,
    backend: MemoryBackend,
}

impl Server {
    pub fn new(config: Config, backend: MemoryBackend) -> Self {
        Self { config, backend }
    }

    pub async fn run(&self) -> Result<()> {
        println!("ChronoMind Server running on {}:{}", self.config.host, self.config.port);
        Ok(())
    }

    pub async fn save_memory(&mut self, memory: TemporalVector) -> Result<()> {
        self.backend.insert_memory(memory).await
    }

    pub async fn get_memory(&self, id: &str) -> Result<Option<TemporalVector>> {
        self.backend.get_memory(id).await
    }

    pub async fn search_similar(&self, query: &[f32], limit: usize) -> Result<Vec<(TemporalVector, f32)>> {
        // Convert query to Vector
        let query_vector = Vector::new(
            "query".to_string(),
            query.to_vec(),
        );

        // Create TemporalVector for search
        let query_temporal = TemporalVector::new(
            query_vector,
            MemoryAttributes {
                timestamp: SystemTime::now(),
                importance: 1.0,
                context: "search".to_string(),
                decay_rate: 0.1,
                relationships: Vec::new(),
                access_count: 0,
                last_access: SystemTime::now(),
            },
        );
        
        // Get memories in the same context and sort by similarity
        let memories = self.backend.search_by_context("search", limit * 2).await?;
        let mut results: Vec<_> = memories.into_iter()
            .map(|memory| {
                let similarity = cosine_similarity(&query_temporal.vector.data, &memory.vector.data);
                (memory, similarity)
            })
            .collect();
            
        // Sort by similarity (highest first) and take top k
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(results.into_iter().take(limit).collect())
    }

    pub async fn update_memory_decay(&mut self) -> Result<()> {
        let duration = std::time::Duration::from_secs(3600); // 1 hour
        self.backend.apply_decay(duration).await
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (norm_a * norm_b)
}
