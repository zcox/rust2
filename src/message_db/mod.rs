//! Message DB Client Library for Rust
//!
//! This library provides a high-performance, async client for Message DB,
//! a PostgreSQL-based event store and message store designed for microservices,
//! event sourcing, and pub/sub architectures.
//!
//! # Quick Start
//!
//! ```no_run
//! use rust2::message_db::{MessageDbClient, MessageDbConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MessageDbConfig::from_connection_string(
//!         "postgresql://postgres:password@localhost:5432/message_store"
//!     )?;
//!
//!     let client = MessageDbClient::new(config).await?;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod connection;
pub mod consumer;
pub mod error;
pub mod operations;
pub mod transaction;
pub mod types;
pub mod utils;

// Re-export main types for convenience
pub use client::MessageDbClient;
pub use connection::MessageDbConfig;
pub use error::{Error, Result};
pub use operations::{CategoryReadOptions, StreamReadOptions};
pub use transaction::Transaction;
pub use types::{Message, WriteMessage};
pub use utils::{category, cardinal_id, get_base_category, get_category_types, id, is_category};
