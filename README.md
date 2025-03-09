# rs-mongo-stream
A MongoDB change stream library for Rust applications

[![GitHub last commit](https://img.shields.io/github/last-commit/lsh0x/rs-mongo-stream)](https://github.com/lsh0x/rs-mongo-stream/commits/main)
[![CI](https://github.com/lsh0x/rs-mongo-stream/workflows/CI/badge.svg)](https://github.com/lsh0x/rs-mongo-stream/actions)
[![Codecov](https://codecov.io/gh/lsh0x/rs-mongo-stream/branch/main/graph/badge.svg)](https://codecov.io/gh/lsh0x/rs-mongo-stream)
[![Docs](https://docs.rs/rs-mongo-stream/badge.svg)](https://docs.rs/rs-mongo-stream)
[![Crates.io](https://img.shields.io/crates/v/rs-mongo-stream.svg)](https://crates.io/crates/rs-mongo-stream)
[![crates.io](https://img.shields.io/crates/d/rs-mongo-stream)](https://crates.io/crates/rs-mongo-stream)


## Overview

rs-mongo-stream is a Rust library that makes it easy to work with MongoDB change streams. It provides a simple interface to monitor and react to changes in your MongoDB collections by registering callbacks for different database events (insertions, updates, and deletions).

## Features

- **Event-based architecture**: Register callbacks for insert, update, and delete operations
- **Asynchronous support**: Built with Tokio for fully asynchronous operation
- **Automatic reconnection**: Handles stream disruptions gracefully
- **Implements Stream trait**: Can be used with standard Rust stream operations
- **Type-safe callbacks**: Strong typing for your event handlers

## Installation

Add the crate to your Cargo.toml:

```toml
[dependencies]
rs-mongo-stream = "0.3.1"
mongodb = "2.4.0"
tokio = { version = "1", features = ["full"] }
```

## Usage Example

```rust
use mongodb::{Client, options::ClientOptions};
use rs_mongo_stream::{MongoStream, Event};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to MongoDB
    let client_options = ClientOptions::parse("mongodb://localhost:27017").await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("my_database");
    
    // Create the stream monitor
    let mut stream = MongoStream::new(db);
    
    // Register callbacks for a collection
    stream.add_callback("users", Event::Insert, |doc| {
        Box::pin(async move {
            println!("New user inserted: {:?}", doc);
            // Handle the insertion...
        })
    });
    
    stream.add_callback("users", Event::Update, |doc| {
        Box::pin(async move {
            println!("User updated: {:?}", doc);
            // Handle the update...
        })
    });
    
    stream.add_callback("users", Event::Delete, |doc| {
        Box::pin(async move {
            println!("User deleted: {:?}", doc);
            // Handle the deletion...
        })
    });
    
    // Start monitoring the collection
    stream.start_stream("users").await?;
    
    Ok(())
}
```

## Error Handling

The library provides a custom `MongoStreamError` type that wraps errors from the MongoDB driver and adds context when needed.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
