use vector_store::{
    core::{
        config::MemoryConfig,
        error::MemoryError,
    },
    memory::types::{MemoryAttributes, TemporalVector, Vector},
    utils::validation::{validate_vector_dimensions, validate_vector_data, validate_temporal_vector},
};
use std::time::SystemTime;

#[tokio::test]
async fn test_validate_vector_dimensions() {
    // Create a test config with specific dimensions
    let config = MemoryConfig {
        max_dimensions: 4,
        ..MemoryConfig::default()
    };

    // Test valid dimensions
    let valid_vector = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let result = validate_vector_dimensions(&valid_vector, &config);
    assert!(result.is_ok());

    // Test invalid dimensions (too few)
    let invalid_vector_too_few = Vector::new(
        "test_vector_too_few".to_string(),
        vec![0.1, 0.2, 0.3],
    );
    let result = validate_vector_dimensions(&invalid_vector_too_few, &config);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidDimensions { got, expected }) => {
            assert_eq!(got, 3);
            assert_eq!(expected, 4);
        }
        _ => panic!("Expected InvalidDimensions error"),
    }

    // Test invalid dimensions (too many)
    let invalid_vector_too_many = Vector::new(
        "test_vector_too_many".to_string(),
        vec![0.1, 0.2, 0.3, 0.4, 0.5],
    );
    let result = validate_vector_dimensions(&invalid_vector_too_many, &config);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidDimensions { got, expected }) => {
            assert_eq!(got, 5);
            assert_eq!(expected, 4);
        }
        _ => panic!("Expected InvalidDimensions error"),
    }
}

#[tokio::test]
async fn test_validate_vector_data() {
    // Test valid vector data
    let valid_vector = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let result = validate_vector_data(&valid_vector);
    assert!(result.is_ok());

    // Test empty vector data
    let empty_vector = Vector::new(
        "empty_vector".to_string(),
        vec![],
    );
    let result = validate_vector_data(&empty_vector);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidVectorData(msg)) => {
            assert_eq!(msg, "Vector data cannot be empty");
        }
        _ => panic!("Expected InvalidVectorData error"),
    }

    // Test vector with non-finite values
    let nan_vector = Vector::new(
        "nan_vector".to_string(),
        vec![0.1, f32::NAN, 0.3, 0.4],
    );
    let result = validate_vector_data(&nan_vector);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidVectorData(msg)) => {
            assert_eq!(msg, "Vector data contains non-finite values");
        }
        _ => panic!("Expected InvalidVectorData error"),
    }

    // Test vector with infinity
    let inf_vector = Vector::new(
        "inf_vector".to_string(),
        vec![0.1, f32::INFINITY, 0.3, 0.4],
    );
    let result = validate_vector_data(&inf_vector);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidVectorData(msg)) => {
            assert_eq!(msg, "Vector data contains non-finite values");
        }
        _ => panic!("Expected InvalidVectorData error"),
    }
}

#[tokio::test]
async fn test_validate_temporal_vector() {
    // Test valid temporal vector
    let valid_vector = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let valid_temporal = TemporalVector::new(
        valid_vector,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 0.5,
            context: "test_context".to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );
    let result = validate_temporal_vector(&valid_temporal);
    assert!(result.is_ok());

    // Test invalid importance (too high)
    let invalid_importance_high = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let invalid_temporal_high = TemporalVector::new(
        invalid_importance_high,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 1.5, // Invalid: > 1.0
            context: "test_context".to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );
    let result = validate_temporal_vector(&invalid_temporal_high);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidImportance(importance)) => {
            assert_eq!(importance, 1.5);
        }
        _ => panic!("Expected InvalidImportance error"),
    }

    // Test invalid importance (negative)
    let invalid_importance_neg = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let invalid_temporal_neg = TemporalVector::new(
        invalid_importance_neg,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: -0.5, // Invalid: < 0.0
            context: "test_context".to_string(),
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );
    let result = validate_temporal_vector(&invalid_temporal_neg);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidImportance(importance)) => {
            assert_eq!(importance, -0.5);
        }
        _ => panic!("Expected InvalidImportance error"),
    }

    // Test invalid decay rate (too high)
    let invalid_decay_high = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let invalid_temporal_decay_high = TemporalVector::new(
        invalid_decay_high,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 0.5,
            context: "test_context".to_string(),
            decay_rate: 1.5, // Invalid: > 1.0
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );
    let result = validate_temporal_vector(&invalid_temporal_decay_high);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidAttributes(msg)) => {
            assert!(msg.contains("Decay rate must be between 0 and 1"));
        }
        _ => panic!("Expected InvalidAttributes error"),
    }

    // Test invalid decay rate (negative)
    let invalid_decay_neg = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let invalid_temporal_decay_neg = TemporalVector::new(
        invalid_decay_neg,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 0.5,
            context: "test_context".to_string(),
            decay_rate: -0.1, // Invalid: < 0.0
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );
    let result = validate_temporal_vector(&invalid_temporal_decay_neg);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidAttributes(msg)) => {
            assert!(msg.contains("Decay rate must be between 0 and 1"));
        }
        _ => panic!("Expected InvalidAttributes error"),
    }

    // Test empty context
    let empty_context = Vector::new(
        "test_vector".to_string(),
        vec![0.1, 0.2, 0.3, 0.4],
    );
    let invalid_temporal_context = TemporalVector::new(
        empty_context,
        MemoryAttributes {
            timestamp: SystemTime::now(),
            importance: 0.5,
            context: "".to_string(), // Invalid: empty context
            decay_rate: 0.1,
            relationships: Vec::new(),
            access_count: 0,
            last_access: SystemTime::now(),
        },
    );
    let result = validate_temporal_vector(&invalid_temporal_context);
    assert!(result.is_err());
    match result {
        Err(MemoryError::InvalidAttributes(msg)) => {
            assert_eq!(msg, "Context cannot be empty");
        }
        _ => panic!("Expected InvalidAttributes error"),
    }
}
