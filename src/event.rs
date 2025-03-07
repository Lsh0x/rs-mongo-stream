//! Event types for MongoDB change stream operations.
//!
//! This module defines the various event types that can be subscribed to
//! from MongoDB change streams.

use mongodb::change_stream::event::OperationType;

/// Represents the types of events that can occur in a MongoDB collection.
///
/// These event types correspond to the MongoDB change stream operation types
/// but are simplified to include only the most common operations.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Event {
    /// Document insertion event
    Insert,
    /// Document update event
    Update,
    /// Document deletion event
    Delete,
}

impl Event {
    /// Returns a string representation of the event type.
    ///
    /// This is useful for logging and debugging purposes.
    ///
    /// # Returns
    ///
    /// A static string representation of the event.
    pub fn event_type_str(&self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

// Conversion from MongoDB's OperationType
impl From<OperationType> for Event {
    fn from(op_type: OperationType) -> Self {
        match op_type {
            OperationType::Insert => Event::Insert,
            OperationType::Update => Event::Update,
            OperationType::Delete => Event::Delete,
            // Default to Insert for unsupported types
            // A more robust implementation could return Result<Event, Error>
            _ => Event::Insert,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::change_stream::event::OperationType;

    #[test]
    fn test_event_type_str() {
        assert_eq!(Event::Insert.event_type_str(), "insert");
        assert_eq!(Event::Update.event_type_str(), "update");
        assert_eq!(Event::Delete.event_type_str(), "delete");
    }

    #[test]
    fn test_from_operation_type() {
        assert_eq!(Event::from(OperationType::Insert), Event::Insert);
        assert_eq!(Event::from(OperationType::Update), Event::Update);
        assert_eq!(Event::from(OperationType::Delete), Event::Delete);
    }

    #[test]
    fn test_unsupported_operation_type() {
        // Test that unsupported operation types default to Insert
        assert_eq!(Event::from(OperationType::Replace), Event::Insert);
        assert_eq!(Event::from(OperationType::Invalidate), Event::Insert);
    }
}
