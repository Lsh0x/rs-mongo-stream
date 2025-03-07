/// # MongoDB Change Stream Wrapper
///
/// This crate provides a convenient wrapper around MongoDB change streams,
/// allowing for easy subscription to database events with customizable callbacks.
///
/// ## Features
///
/// - Easy subscription to MongoDB change events (inserts, updates, deletes)
/// - Support for custom callbacks per event type
/// - Automatic reconnection on errors
/// - Proper resource management with explicit stream closing
///
/// ## Example
///
/// ```rust,ignore
/// use mongodb::{Client, options::ClientOptions};
/// use rs_mongo_stream::{MongoStream, Event};
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     // Connect to MongoDB
///     let client_options = ClientOptions::parse("mongodb://localhost:27017").await?;
///     let client = Client::with_options(client_options)?;
///     let db = client.database("test_db");
///     
///     // Create MongoStream instance
///     let mut mongo_stream = MongoStream::new(db);
///     
///     // Add custom callback for insert events
///     mongo_stream.add_callback("users", Event::Insert, |doc| {
///         Box::pin(async move {
///             println!("Insert event received: {:?}", doc);
///             // Additional processing logic here
///         })
///     });
///     
///     // Start monitoring the collection in the background
///     mongo_stream.start_stream("users").await?;
///     
///     // ... Application logic ...
///     
///     // When done with a specific stream
///     mongo_stream.close_stream("users").await;
///     
///     // Or close all streams when shutting down
///     mongo_stream.close_all_streams().await;
///     
///     Ok(())
/// }
/// ```
mod error;
mod event;
mod stream;

pub use error::MongoStreamError;
pub use event::Event;
pub use stream::{Callbacks, MongoStream};
