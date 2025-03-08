//! Core functionality for MongoDB change stream processing.
//!
//! This module provides the main `MongoStream` type which allows for subscribing
//! to MongoDB change events with custom callbacks.

use mongodb::Database;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt};

use crate::error::MongoStreamError;
use crate::event::Event;
use mongodb::bson::Document;

/// Type alias for a callback function that processes MongoDB documents.
///
/// Callbacks should be async functions that take a Document reference
/// and return nothing.
type CallbackFn = dyn Fn(&Document) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync;

/// A collection of callbacks mapped to their respective event types.
///
/// This type maps Event types to their corresponding callback functions.
pub type Callbacks = HashMap<Event, Arc<CallbackFn>>;

/// The main wrapper around MongoDB change streams.
///
/// `MongoStream` provides an interface for subscribing to MongoDB change events
/// with custom callbacks for different event types per collection.
pub struct MongoStream {
    /// The MongoDB database to monitor
    db: Database,
    /// Maps collection names to their registered callbacks
    collection_callbacks: HashMap<String, Callbacks>,
    /// Active streams that can be closed
    active_streams: HashMap<String, tokio::sync::mpsc::Sender<()>>,
}

impl MongoStream {
    /// Creates a new MongoStream instance for the given database.
    ///
    /// # Arguments
    ///
    /// * `db` - The MongoDB database to monitor.
    ///
    /// # Returns
    ///
    /// A new `MongoStream` instance configured to work with the provided database.
    pub fn new(db: Database) -> Self {
        Self {
            db,
            collection_callbacks: HashMap::new(),
            active_streams: HashMap::new(),
        }
    }

    /// Registers a callback for a specific event type on a collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - The name of the MongoDB collection to monitor.
    /// * `event` - The event type to subscribe to.
    /// * `callback` - The async function to call when the event occurs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use MongoStream::Event;
    ///
    /// // load the mongodb database correctly
    /// let db = Database{};
    /// let mut mongo_stream = MongoStream::new(db: Database);
    /// mongo_stream.add_callback("users", Event::Insert, |event| {
    ///     Box::pin(async move {
    ///         println!("New user inserted: {:?}", event);
    ///     })
    /// });
    /// ```
    pub fn add_callback<F>(&mut self, collection_name: impl Into<String>, event: Event, callback: F)
    where
        F: Fn(&Document) -> Pin<Box<dyn Future<Output = ()> + Send>> + 'static + Send + Sync,
    {
        let collection_name = collection_name.into();
        let callback_arc = Arc::new(callback);
        let default_callbacks = self.create_default_callbacks();

        self.collection_callbacks
            .entry(collection_name)
            .or_insert_with(|| default_callbacks)
            .insert(event, callback_arc);
    }

    /// Creates a set of default callbacks for all event types.
    ///
    /// These callbacks simply log that an event was received.
    ///
    /// # Returns
    ///
    /// A HashMap mapping event types to their default callbacks.
    fn create_default_callbacks(&self) -> Callbacks {
        let mut callbacks: Callbacks = HashMap::new();

        // Create default handlers for each event type
        for event in [Event::Insert, Event::Update, Event::Delete] {
            let event_name = event.event_type_str().to_string();

            callbacks.insert(
                event,
                Arc::new(move |_doc: &Document| {
                    let event_name = event_name.clone();
                    Box::pin(async move {
                        println!("{} event received", event_name);
                    })
                }),
            );
        }

        callbacks
    }

    /// Retrieves the callbacks for a specific collection.
    ///
    /// If no callbacks are registered for the collection, default callbacks are returned.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - The name of the collection to get callbacks for.
    ///
    /// # Returns
    ///
    /// A HashMap mapping event types to their registered callbacks.
    fn get_collection_callbacks(&self, collection_name: &str) -> Callbacks {
        match self.collection_callbacks.get(collection_name) {
            Some(callbacks) => {
                // Clone the callbacks map
                callbacks
                    .iter()
                    .map(|(event, callback)| (*event, Arc::clone(callback)))
                    .collect()
            }
            None => self.create_default_callbacks(),
        }
    }

    /// Starts monitoring a collection for changes.
    ///
    /// This method will block and continuously process events from the MongoDB change stream
    /// until an error occurs or the stream is closed.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - The name of the collection to monitor.
    ///
    /// # Returns
    ///
    /// A Result that is Ok if the stream was closed normally, or an error if something went wrong.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    ///
    /// async fn monitor() -> Result<(), MongoStreamError> {
    ///     let mongo_stream = MongoStream::new(db);
    ///     // ... register callbacks ...
    ///     mongo_stream.start_stream("users").await
    /// }
    /// ```
    /// Starts monitoring a collection for changes.
    ///
    /// This method will spawn a new task to monitor the collection and return immediately.
    /// To stop the stream, call `close_stream` with the same collection name.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - The name of the collection to monitor.
    ///
    /// # Returns
    ///
    /// A Result that is Ok if the stream was started successfully, or an error if something went wrong.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn start_monitoring() -> Result<(), MongoStreamError> {
    ///     let mongo_stream = MongoStream::new(db);
    ///     // ... register callbacks ...
    ///     mongo_stream.start_stream("users").await?;
    ///     // The stream is now running in the background
    ///     
    ///     // Later, when you want to stop it:
    ///     mongo_stream.close_stream("users").await;
    ///     Ok(())
    /// }
    /// ```
    pub async fn start_stream(&mut self, collection_name: &str) -> Result<(), MongoStreamError> {
        // Check if the stream is already running
        if self.active_streams.contains_key(collection_name) {
            return Err(MongoStreamError::new(format!(
                "Stream for collection '{}' is already running",
                collection_name
            )));
        }

        let collection = self
            .db
            .collection::<mongodb::bson::Document>(collection_name);

        // Create a channel to signal stream closure
        let (tx, mut rx) = mpsc::channel::<()>(1);

        // Store the sender in the active_streams map
        self.active_streams.insert(collection_name.to_string(), tx);

        // Get the callbacks
        let callbacks = self.get_collection_callbacks(collection_name);
        let collection_name = collection_name.to_string();
        let db = self.db.clone();

        // Spawn a new task to handle the stream
        tokio::spawn(async move {
            let mut stream = match collection.watch(None, None).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "Failed to start stream for collection '{}': {}",
                        collection_name, e
                    );
                    return;
                }
            };

            loop {
                tokio::select! {
                    // Check if we've received a signal to close the stream
                    _ = rx.recv() => {
                        println!("Closing stream for collection '{}'", collection_name);
                        break;
                    }
                    // Process the next event from the stream
                    next_event = stream.next() => {
                        match next_event {
                            Some(result) => {
                                match result {
                                    Ok(change_stream_event) => {
                                        let event_type = Event::from(change_stream_event.operation_type);
                                        // Find and execute the appropriate callback
                                        if let Some(callback) = callbacks.get(&event_type) {
                                            if let Some(doc) = change_stream_event.full_document {
                                                callback(&doc).await;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Error in MongoDB stream for collection '{}': {}. Reconnecting...", collection_name, e);
                                        // Graceful reconnection
                                        match db.collection::<mongodb::bson::Document>(&collection_name).watch(None, None).await {
                                            Ok(new_stream) => stream = new_stream,
                                            Err(reconnect_err) => {
                                                eprintln!("Failed to reconnect to collection '{}': {}", collection_name, reconnect_err);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                // Stream has ended normally
                                println!("Stream for collection '{}' has ended", collection_name);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Closes a specific stream for a collection.
    ///
    /// This method sends a signal to stop the monitoring task for a specific collection.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - The name of the collection whose stream should be closed.
    ///
    /// # Returns
    ///
    /// A boolean indicating whether a stream was found and closed. Returns `false` if
    /// there was no active stream for the given collection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use rs_mongo_stream::MongoStream;
    ///
    /// async fn example() {
    ///     let mongo_stream = MongoStream::new(db);
    ///     // ... start streams ...
    ///     
    ///     // When done with a specific collection
    ///     let closed = mongo_stream.close_stream("users").await;
    ///     println!("Stream closed: {}", closed);
    /// }
    /// ```
    pub async fn close_stream(&mut self, collection_name: &str) -> bool {
        if let Some(tx) = self.active_streams.remove(collection_name) {
            // Send signal to close the stream
            // It's okay if this fails - the receiver might have been dropped already
            let _ = tx.send(()).await;
            true
        } else {
            false
        }
    }

    /// Closes all active streams.
    ///
    /// This method sends signals to stop all monitoring tasks and clears the active streams list.
    ///
    /// # Returns
    ///
    /// The number of streams that were closed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn shutdown() {
    ///     let mongo_stream = MongoStream::new(db);
    ///     // ... start streams ...
    ///     
    ///     // When shutting down the application
    ///     let closed_count = mongo_stream.close_all_streams().await;
    ///     println!("Closed {} streams", closed_count);
    /// }
    /// ```
    pub async fn close_all_streams(&mut self) -> usize {
        let count = self.active_streams.len();

        // Send close signal to all active streams
        for (collection_name, tx) in self.active_streams.drain() {
            let _ = tx.send(()).await;
            println!(
                "Sent close signal to stream for collection '{}'",
                collection_name
            );
        }

        count
    }
}

/// Implementation of Stream trait for MongoStream to make it compatible with tokio_stream.
///
/// Note: This is a placeholder implementation and would need to be expanded
/// in a real-world scenario to connect to actual MongoDB change streams.
impl Stream for MongoStream {
    type Item = Result<Event, MongoStreamError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Placeholder implementation for the Stream trait
        // In a real implementation, this would be connected to MongoDB change streams
        Poll::Ready(Some(Ok(Event::Insert)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::{bson::Document, options::ClientOptions, Client};
    use tokio::sync::mpsc;

    async fn setup_test_db() -> Database {
        let client_options = ClientOptions::parse("mongodb://localhost:27017")
            .await
            .unwrap();
        let client = Client::with_options(client_options).unwrap();
        client.database("test_db")
    }

    #[tokio::test]
    async fn test_mongo_stream_creation() {
        let db = setup_test_db().await;
        let mongo_stream = MongoStream::new(db.clone());

        assert!(mongo_stream.collection_callbacks.is_empty());
        assert!(mongo_stream.active_streams.is_empty());
    }

    #[tokio::test]
    async fn test_add_callback() {
        let db = setup_test_db().await;
        let mut mongo_stream = MongoStream::new(db);

        let (tx, mut rx) = mpsc::channel(1);
        mongo_stream.add_callback("test_collection", Event::Insert, move |_doc| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        });

        let callbacks = mongo_stream.get_collection_callbacks("test_collection");
        assert!(callbacks.contains_key(&Event::Insert));

        // Execute the callback
        if let Some(callback) = callbacks.get(&Event::Insert) {
            let doc = Document::new();
            callback(&doc).await;
            assert!(rx.recv().await.is_some());
        }
    }

    #[tokio::test]
    async fn test_start_and_close_stream() {
        let db = setup_test_db().await;
        let mut mongo_stream = MongoStream::new(db);

        // Test starting stream
        assert!(mongo_stream.start_stream("test_collection").await.is_ok());
        assert!(mongo_stream.active_streams.contains_key("test_collection"));

        // Test closing stream
        assert!(mongo_stream.close_stream("test_collection").await);
        assert!(!mongo_stream.active_streams.contains_key("test_collection"));
    }

    #[tokio::test]
    async fn test_close_all_streams() {
        let db = setup_test_db().await;
        let mut mongo_stream = MongoStream::new(db);

        mongo_stream.start_stream("collection1").await.unwrap();
        mongo_stream.start_stream("collection2").await.unwrap();

        let closed_count = mongo_stream.close_all_streams().await;
        assert_eq!(closed_count, 2);
        assert!(mongo_stream.active_streams.is_empty());
    }

    #[tokio::test]
    async fn test_default_callbacks() {
        let db = setup_test_db().await;
        let mongo_stream = MongoStream::new(db);

        let callbacks = mongo_stream.get_collection_callbacks("test_collection");
        assert_eq!(callbacks.len(), 3);
        assert!(callbacks.contains_key(&Event::Insert));
        assert!(callbacks.contains_key(&Event::Update));
        assert!(callbacks.contains_key(&Event::Delete));
    }

    #[tokio::test]
    async fn test_double_start_error() {
        let db = setup_test_db().await;
        let mut mongo_stream = MongoStream::new(db);

        mongo_stream.start_stream("test_collection").await.unwrap();
        let result = mongo_stream.start_stream("test_collection").await;

        assert!(matches!(result, Err(MongoStreamError { .. })));
    }
}
