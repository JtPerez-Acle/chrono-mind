use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use crate::error::Result;

/// Represents the data directory structure for vector storage
pub struct DataDirectory {
    root: PathBuf,
    vectors_dir: PathBuf,
    index_dir: PathBuf,
    metadata_dir: PathBuf,
}

impl DataDirectory {
    /// Creates a new data directory structure
    pub fn create(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let vectors_dir = root.join("vectors");
        let index_dir = root.join("index");
        let metadata_dir = root.join("metadata");

        info!(path = %root.display(), "Creating data directory structure");

        // Create directory structure
        fs::create_dir_all(&root)?;
        fs::create_dir_all(&vectors_dir)?;
        fs::create_dir_all(&index_dir)?;
        fs::create_dir_all(&metadata_dir)?;

        Ok(Self {
            root,
            vectors_dir,
            index_dir,
            metadata_dir,
        })
    }

    /// Opens an existing data directory
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let vectors_dir = root.join("vectors");
        let index_dir = root.join("index");
        let metadata_dir = root.join("metadata");

        info!(path = %root.display(), "Opening data directory");

        // Verify directory structure exists
        if !root.exists() || !vectors_dir.exists() || !index_dir.exists() || !metadata_dir.exists() {
            return Err(crate::error::VectorStoreError::Storage(
                "Invalid data directory structure".into(),
            ));
        }

        Ok(Self {
            root,
            vectors_dir,
            index_dir,
            metadata_dir,
        })
    }

    /// Returns the path to store vector data for a given ID
    pub fn vector_path(&self, id: &str) -> PathBuf {
        self.vectors_dir.join(format!("{}.vec", id))
    }

    /// Returns the path to store index data for a given ID
    pub fn index_path(&self, id: &str) -> PathBuf {
        self.index_dir.join(format!("{}.idx", id))
    }

    /// Returns the path to store metadata for a given ID
    pub fn metadata_path(&self, id: &str) -> PathBuf {
        self.metadata_dir.join(format!("{}.meta", id))
    }

    /// Returns the root directory path
    pub fn root_path(&self) -> &Path {
        &self.root
    }

    /// Returns the vectors directory path
    pub fn vectors_path(&self) -> &Path {
        &self.vectors_dir
    }

    /// Returns the index directory path
    pub fn index_path_dir(&self) -> &Path {
        &self.index_dir
    }

    /// Returns the metadata directory path
    pub fn metadata_path_dir(&self) -> &Path {
        &self.metadata_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use test_log::test;

    #[test]
    fn test_data_directory_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let data_dir = DataDirectory::create(temp_dir.path())?;

        assert!(data_dir.root_path().exists());
        assert!(data_dir.vectors_path().exists());
        assert!(data_dir.index_path_dir().exists());
        assert!(data_dir.metadata_path_dir().exists());

        Ok(())
    }

    #[test]
    fn test_data_directory_paths() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let data_dir = DataDirectory::create(temp_dir.path())?;

        let id = "test_vector";
        assert_eq!(
            data_dir.vector_path(id),
            data_dir.vectors_path().join("test_vector.vec")
        );
        assert_eq!(
            data_dir.index_path(id),
            data_dir.index_path_dir().join("test_vector.idx")
        );
        assert_eq!(
            data_dir.metadata_path(id),
            data_dir.metadata_path_dir().join("test_vector.meta")
        );

        Ok(())
    }

    #[test]
    fn test_data_directory_open() -> Result<()> {
        let temp_dir = TempDir::new()?;
        
        // Create directory structure
        {
            let _data_dir = DataDirectory::create(temp_dir.path())?;
        }

        // Open existing directory
        let data_dir = DataDirectory::open(temp_dir.path())?;
        assert!(data_dir.root_path().exists());
        assert!(data_dir.vectors_path().exists());
        assert!(data_dir.index_path_dir().exists());
        assert!(data_dir.metadata_path_dir().exists());

        Ok(())
    }

    #[test]
    fn test_data_directory_open_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let result = DataDirectory::open(temp_dir.path());
        assert!(result.is_err());
    }
}
