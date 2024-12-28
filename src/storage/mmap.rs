use std::fs::OpenOptions;
use std::path::Path;
use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Result, VectorStoreError};
use super::{Vector, VectorStorage};
use super::metrics::{DistanceMetric, EuclideanDistance};

const HEADER_SIZE: usize = 16; // 4 bytes for magic + 4 for version + 8 for vector count
const MAGIC: u32 = 0x5653544F; // "VSTO" in ASCII
const VERSION: u32 = 1;

#[derive(Debug)]
pub struct MmapVectorStorage {
    mmap: MmapMut,
    metric: Box<dyn DistanceMetric>,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct VectorHeader {
    id_length: u32,
    data_length: u32,
    metadata_length: u32,
}

impl MmapVectorStorage {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        info!(path = %path_str, "Creating new memory-mapped vector storage");
        
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
            
        // Initialize with minimum size
        file.set_len(HEADER_SIZE as u64)?;
        
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        
        // Write header
        mmap[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        mmap[4..8].copy_from_slice(&VERSION.to_le_bytes());
        mmap[8..16].copy_from_slice(&0u64.to_le_bytes()); // Initial vector count
        
        Ok(Self {
            mmap,
            metric: Box::new(EuclideanDistance),
            path: path_str,
        })
    }
    
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        info!(path = %path_str, "Opening existing memory-mapped vector storage");
        
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?;
            
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        
        // Verify header
        if mmap.len() < HEADER_SIZE {
            return Err(VectorStoreError::Storage("Invalid file size".into()));
        }
        
        let magic = u32::from_le_bytes(mmap[0..4].try_into().unwrap());
        let version = u32::from_le_bytes(mmap[4..8].try_into().unwrap());
        
        if magic != MAGIC {
            return Err(VectorStoreError::Storage("Invalid magic number".into()));
        }
        
        if version != VERSION {
            return Err(VectorStoreError::Storage("Unsupported version".into()));
        }
        
        Ok(Self {
            mmap,
            metric: Box::new(EuclideanDistance),
            path: path_str,
        })
    }
    
    fn get_vector_count(&self) -> u64 {
        u64::from_le_bytes(self.mmap[8..16].try_into().unwrap())
    }
    
    fn set_vector_count(&mut self, count: u64) {
        self.mmap[8..16].copy_from_slice(&count.to_le_bytes());
    }
}

#[async_trait::async_trait]
impl VectorStorage for MmapVectorStorage {
    async fn insert(&mut self, vector: Vector) -> Result<()> {
        debug!(id = %vector.id, dimensions = vector.data.len(), "Inserting vector to mmap storage");
        
        // Serialize the vector components
        let id_bytes = vector.id.as_bytes();
        let data_bytes = bincode::serialize(&vector.data)?;
        let metadata_bytes = if let Some(metadata) = vector.metadata {
            serde_json::to_vec(&metadata)?
        } else {
            Vec::new()
        };
        
        let header = VectorHeader {
            id_length: id_bytes.len() as u32,
            data_length: data_bytes.len() as u32,
            metadata_length: metadata_bytes.len() as u32,
        };
        
        let header_bytes = bincode::serialize(&header)?;
        let total_size = header_bytes.len() + id_bytes.len() + data_bytes.len() + metadata_bytes.len();
        
        // Resize mmap if needed
        let current_len = self.mmap.len();
        let required_len = current_len + total_size;
        
        if required_len > current_len {
            warn!(
                current_size = current_len,
                required_size = required_len,
                "Resizing mmap file"
            );
            
            // Create new mapping with larger size
            drop(std::mem::replace(&mut self.mmap, MmapMut::map_anon(1)?)); // Temporary placeholder
            
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.path)?;
                
            file.set_len(required_len as u64)?;
            self.mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        }
        
        // Write vector data
        let mut offset = current_len;
        
        // Write header
        self.mmap[offset..offset + header_bytes.len()].copy_from_slice(&header_bytes);
        offset += header_bytes.len();
        
        // Write ID
        self.mmap[offset..offset + id_bytes.len()].copy_from_slice(id_bytes);
        offset += id_bytes.len();
        
        // Write data
        self.mmap[offset..offset + data_bytes.len()].copy_from_slice(&data_bytes);
        offset += data_bytes.len();
        
        // Write metadata
        if !metadata_bytes.is_empty() {
            self.mmap[offset..offset + metadata_bytes.len()].copy_from_slice(&metadata_bytes);
        }
        
        // Update vector count
        let count = self.get_vector_count();
        self.set_vector_count(count + 1);
        
        debug!("Vector inserted successfully");
        Ok(())
    }
    
    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(Vector, f32)>> {
        debug!(dimensions = query.len(), limit = limit, "Searching vectors in mmap storage");
        
        let mut results = Vec::new();
        let mut offset = HEADER_SIZE;
        let count = self.get_vector_count();
        
        for _ in 0..count {
            if offset >= self.mmap.len() {
                break;
            }
            
            // Read header
            let header_size = std::mem::size_of::<VectorHeader>();
            let header: VectorHeader = bincode::deserialize(&self.mmap[offset..offset + header_size])?;
            offset += header_size;
            
            // Read ID
            let id = String::from_utf8(self.mmap[offset..offset + header.id_length as usize].to_vec())?;
            offset += header.id_length as usize;
            
            // Read data
            let data: Vec<f32> = bincode::deserialize(&self.mmap[offset..offset + header.data_length as usize])?;
            offset += header.data_length as usize;
            
            // Read metadata
            let metadata = if header.metadata_length > 0 {
                let metadata_bytes = &self.mmap[offset..offset + header.metadata_length as usize];
                Some(serde_json::from_slice(metadata_bytes)?)
            } else {
                None
            };
            offset += header.metadata_length as usize;
            
            let vector = Vector { id, data, metadata };
            let distance = self.metric.distance(&vector.data, query);
            
            results.push((vector, distance));
        }
        
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(limit);
        
        debug!(found = results.len(), "Search completed");
        Ok(results)
    }
    
    async fn delete(&mut self, _id: &str) -> Result<()> {
        warn!("Delete operation is not supported in memory-mapped storage");
        Err(VectorStoreError::Storage("Delete operation not supported".into()))
    }
    
    async fn get(&self, id: &str) -> Result<Option<Vector>> {
        debug!(id = %id, "Getting vector from mmap storage");
        
        let mut offset = HEADER_SIZE;
        let count = self.get_vector_count();
        
        for _ in 0..count {
            if offset >= self.mmap.len() {
                break;
            }
            
            // Read header
            let header_size = std::mem::size_of::<VectorHeader>();
            let header: VectorHeader = bincode::deserialize(&self.mmap[offset..offset + header_size])?;
            offset += header_size;
            
            // Read ID
            let current_id = String::from_utf8(self.mmap[offset..offset + header.id_length as usize].to_vec())?;
            
            if current_id == id {
                offset += header.id_length as usize;
                
                // Read data
                let data: Vec<f32> = bincode::deserialize(&self.mmap[offset..offset + header.data_length as usize])?;
                offset += header.data_length as usize;
                
                // Read metadata
                let metadata = if header.metadata_length > 0 {
                    let metadata_bytes = &self.mmap[offset..offset + header.metadata_length as usize];
                    Some(serde_json::from_slice(metadata_bytes)?)
                } else {
                    None
                };
                
                return Ok(Some(Vector {
                    id: current_id,
                    data,
                    metadata,
                }));
            }
            
            offset += header.id_length as usize + header.data_length as usize + header.metadata_length as usize;
        }
        
        Ok(None)
    }
    
    async fn len(&self) -> Result<usize> {
        Ok(self.get_vector_count() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_mmap_storage_basic_operations() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let mut storage = MmapVectorStorage::create(temp_file.path())?;
        
        // Test insert and get
        let vector = Vector {
            id: "test1".to_string(),
            data: vec![1.0, 2.0, 3.0],
            metadata: None,
        };
        
        storage.insert(vector.clone()).await?;
        assert_eq!(storage.len().await?, 1);
        
        let retrieved = storage.get("test1").await?.unwrap();
        assert_eq!(retrieved.data, vector.data);
        
        Ok(())
    }

    #[test(tokio::test)]
    async fn test_mmap_storage_search() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let mut storage = MmapVectorStorage::create(temp_file.path())?;
        
        // Insert test vectors
        let vectors = vec![
            Vector {
                id: "1".to_string(),
                data: vec![1.0, 0.0, 0.0],
                metadata: None,
            },
            Vector {
                id: "2".to_string(),
                data: vec![0.0, 1.0, 0.0],
                metadata: None,
            },
            Vector {
                id: "3".to_string(),
                data: vec![0.0, 0.0, 1.0],
                metadata: None,
            },
        ];
        
        for v in vectors {
            storage.insert(v).await?;
        }
        
        // Search for nearest vector
        let results = storage.search(&[1.0, 0.0, 0.0], 1).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "1");
        
        Ok(())
    }

    #[test(tokio::test)]
    async fn test_mmap_storage_persistence() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let temp_path = temp_file.path().to_owned();
        
        // Create and populate storage
        {
            let mut storage = MmapVectorStorage::create(&temp_path)?;
            
            let vector = Vector {
                id: "test1".to_string(),
                data: vec![1.0, 2.0, 3.0],
                metadata: Some(serde_json::json!({"key": "value"})),
            };
            
            storage.insert(vector).await?;
        }
        
        // Reopen storage and verify data
        let storage = MmapVectorStorage::open(&temp_path)?;
        assert_eq!(storage.len().await?, 1);
        
        let vector = storage.get("test1").await?.unwrap();
        assert_eq!(vector.data, vec![1.0, 2.0, 3.0]);
        assert_eq!(
            vector.metadata.unwrap(),
            serde_json::json!({"key": "value"})
        );
        
        Ok(())
    }
}
