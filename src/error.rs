use std::io;
use std::string::FromUtf8Error;
use thiserror::Error;
use anyhow::Error;
use serde_json::Error as SerdeJsonError;
use bincode::ErrorKind;

#[derive(Error, Debug)]
pub enum VectorStoreError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerdeJsonError),
    
    #[error("Bincode error: {0}")]
    Bincode(#[from] Box<ErrorKind>),
    
    #[error("Vector with id {0} not found")]
    NotFound(String),
    
    #[error("Vector dimensions mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Index error: {0}")]
    Index(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<bincode::Error> for VectorStoreError {
    fn from(err: bincode::Error) -> Self {
        VectorStoreError::Bincode(Box::new(err.into_kind()))
    }
}

impl From<std::string::FromUtf8Error> for VectorStoreError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        VectorStoreError::InvalidConfig(err.to_string())
    }
}

impl From<anyhow::Error> for VectorStoreError {
    fn from(err: anyhow::Error) -> Self {
        VectorStoreError::Other(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VectorStoreError>;
