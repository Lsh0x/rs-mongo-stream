//! Error types for MongoDB change stream operations.
//!
//! This module provides a unified error type for all operations in the crate.

use std::{error::Error, fmt};

/// Represents errors that can occur when working with MongoDB change streams.
///
/// This type wraps the various error types that might be encountered when
/// interacting with MongoDB, providing a unified error handling approach.
#[derive(Debug)]
pub struct MongoStreamError {
    /// The error message
    message: String,
}

impl MongoStreamError {
    /// Creates a new error with the given message.
    ///
    /// # Arguments
    ///
    /// * `message` - A string describing the error.
    ///
    /// # Returns
    ///
    /// A new `MongoStreamError` instance.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

// Standard Error trait implementation
impl Error for MongoStreamError {}

// Display implementation for human-readable error messages
impl fmt::Display for MongoStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MongoStream error: {}", self.message)
    }
}

// Conversion from MongoDB's error type
impl From<mongodb::error::Error> for MongoStreamError {
    fn from(error: mongodb::error::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

// Conversion from other error types that might be encountered
impl From<tokio::sync::mpsc::error::SendError<()>> for MongoStreamError {
    fn from(error: tokio::sync::mpsc::error::SendError<()>) -> Self {
        Self {
            message: format!("Failed to send close signal: {}", error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::error::SendError;

    #[test]
    fn test_new_error() {
        let err = MongoStreamError::new("test error");
        assert_eq!(err.message, "test error");
    }

    #[test]
    fn test_display_formatting() {
        let err = MongoStreamError::new("test error");
        assert_eq!(format!("{}", err), "MongoStream error: test error");
    }

    #[test]
    fn test_error_trait() {
        let err = MongoStreamError::new("test error");
        assert!(err.source().is_none());
    }

    #[test]
    fn test_from_send_error() {
        let send_err = SendError(());
        let err: MongoStreamError = send_err.into();
        assert!(err.message.contains("Failed to send close signal"));
    }
}
