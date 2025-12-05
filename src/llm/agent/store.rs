//! Event store for thread persistence using MessageDB
//!
//! This module provides `ThreadStore`, which handles reading and writing
//! ThreadEvent instances to MessageDB streams.
//!
//! Key concepts:
//! - Each thread is a stream: `thread:v0-{threadId}`
//! - Events are stored as messages in the stream
//! - Optimistic concurrency control prevents conflicts
//! - Batch writes for efficiency

use crate::llm::agent::events::{thread_stream_name, ThreadEvent};
use crate::message_db::{
    client::MessageDbClient,
    error::{Error, Result},
    operations::StreamReadOptions,
    types::Message,
};
use serde_json::Value;

/// Thread event store backed by MessageDB
///
/// Provides persistent storage for thread events with:
/// - Stream-based storage (one stream per thread)
/// - Optimistic concurrency control
/// - Batch write support
/// - Event reconstruction from storage
///
/// # Example
///
/// ```no_run
/// use rust2::llm::agent::store::ThreadStore;
/// use rust2::llm::agent::events::{ThreadEvent, UserMessageReceivedData};
/// use rust2::message_db::{MessageDbClient, MessageDbConfig};
/// use chrono::Utc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = MessageDbConfig::from_connection_string(
///         "postgresql://postgres:password@localhost:5433/message_store"
///     )?;
///     let client = MessageDbClient::new(config).await?;
///     let store = ThreadStore::new(client);
///
///     // Append an event
///     let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
///         message: "Hello!".to_string(),
///         timestamp: Utc::now(),
///     });
///     let version = store.append_event("thread-123", event, None, None).await?;
///
///     // Read all events
///     let events = store.read_thread_events("thread-123").await?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ThreadStore {
    client: MessageDbClient,
}

impl ThreadStore {
    /// Create a new ThreadStore
    pub fn new(client: MessageDbClient) -> Self {
        Self { client }
    }

    /// Read all events for a thread
    ///
    /// Returns events in chronological order (oldest first).
    /// Returns an empty vector if the thread doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The unique thread identifier
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::llm::agent::store::ThreadStore;
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = MessageDbConfig::from_connection_string("postgresql://postgres:password@localhost:5433/message_store")?;
    /// # let client = MessageDbClient::new(config).await?;
    /// # let store = ThreadStore::new(client);
    /// let events = store.read_thread_events("thread-123").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_thread_events(&self, thread_id: &str) -> Result<Vec<ThreadEvent>> {
        let stream_name = thread_stream_name(thread_id);

        let options = StreamReadOptions::new(&stream_name).with_batch_size(1000);

        let messages = self.client.get_stream_messages(options).await?;

        // Convert MessageDB Messages to ThreadEvents
        messages
            .into_iter()
            .map(|msg| self.message_to_thread_event(msg))
            .collect()
    }

    /// Append a single event to a thread
    ///
    /// Returns the new stream version (position) after the write.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The unique thread identifier
    /// * `event` - The event to append
    /// * `expected_version` - Optional expected version for optimistic concurrency control
    /// * `metadata` - Optional metadata to attach to the event
    ///
    /// # Optimistic Concurrency
    ///
    /// Pass `expected_version` to ensure no concurrent writes have occurred:
    /// - `None` - No concurrency check (always succeeds)
    /// - `Some(n)` - Fails if stream version is not exactly `n`
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::llm::agent::store::ThreadStore;
    /// # use rust2::llm::agent::events::{ThreadEvent, UserMessageReceivedData};
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use chrono::Utc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = MessageDbConfig::from_connection_string("postgresql://postgres:password@localhost:5433/message_store")?;
    /// # let client = MessageDbClient::new(config).await?;
    /// # let store = ThreadStore::new(client);
    /// let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
    ///     message: "Hello!".to_string(),
    ///     timestamp: Utc::now(),
    /// });
    ///
    /// // Write without concurrency check
    /// let version = store.append_event("thread-123", event.clone(), None, None).await?;
    ///
    /// // Write with concurrency check
    /// let next_version = store.append_event("thread-123", event, Some(version), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn append_event(
        &self,
        thread_id: &str,
        event: ThreadEvent,
        expected_version: Option<i64>,
        metadata: Option<Value>,
    ) -> Result<i64> {
        let stream_name = thread_stream_name(thread_id);

        let mut write_msg = event.to_write_message(&stream_name, metadata);

        // Add expected version for optimistic concurrency control
        if let Some(version) = expected_version {
            write_msg = write_msg.with_expected_version(version);
        }

        self.client.write_message(write_msg).await
    }

    /// Append multiple events to a thread in a batch
    ///
    /// All events are written atomically within a transaction.
    /// Returns the final stream version after all writes.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The unique thread identifier
    /// * `events` - Vector of events to append
    /// * `expected_version` - Optional expected version for the first write
    /// * `metadata` - Optional metadata to attach to all events
    ///
    /// # Optimistic Concurrency
    ///
    /// The `expected_version` applies to the first event write.
    /// Subsequent events in the batch will automatically use incremented versions.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::llm::agent::store::ThreadStore;
    /// # use rust2::llm::agent::events::{ThreadEvent, AgentIterationStartedData, LlmCallStartedData};
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use chrono::Utc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = MessageDbConfig::from_connection_string("postgresql://postgres:password@localhost:5433/message_store")?;
    /// # let client = MessageDbClient::new(config).await?;
    /// # let store = ThreadStore::new(client);
    /// let events = vec![
    ///     ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
    ///         iteration: 1,
    ///         timestamp: Utc::now(),
    ///     }),
    ///     ThreadEvent::LlmCallStarted(LlmCallStartedData {
    ///         provider: "claude".to_string(),
    ///         model: "sonnet".to_string(),
    ///         message_count: 5,
    ///         timestamp: Utc::now(),
    ///     }),
    /// ];
    ///
    /// let final_version = store.append_events("thread-123", events, None, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn append_events(
        &self,
        thread_id: &str,
        events: Vec<ThreadEvent>,
        expected_version: Option<i64>,
        metadata: Option<Value>,
    ) -> Result<i64> {
        if events.is_empty() {
            return Err(Error::ValidationError(
                "Cannot append empty event list".to_string(),
            ));
        }

        let stream_name = thread_stream_name(thread_id);

        // Start a transaction for atomic batch write
        let mut txn = self.client.begin_transaction().await?;

        let mut current_version = expected_version;
        let mut last_position = 0i64;

        for event in events {
            let mut write_msg = event.to_write_message(&stream_name, metadata.clone());

            // Apply expected version for optimistic concurrency control
            if let Some(version) = current_version {
                write_msg = write_msg.with_expected_version(version);
            }

            // Write the message
            last_position = txn.write_message(write_msg).await?;

            // Increment expected version for next write
            current_version = Some(last_position);
        }

        // Commit transaction
        txn.commit().await?;

        Ok(last_position)
    }

    /// Get the latest version (position) of a thread stream
    ///
    /// Returns `None` if the thread doesn't exist or has no events.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - The unique thread identifier
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::llm::agent::store::ThreadStore;
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = MessageDbConfig::from_connection_string("postgresql://postgres:password@localhost:5433/message_store")?;
    /// # let client = MessageDbClient::new(config).await?;
    /// # let store = ThreadStore::new(client);
    /// if let Some(version) = store.get_stream_version("thread-123").await? {
    ///     println!("Thread is at version {}", version);
    /// } else {
    ///     println!("Thread does not exist or is empty");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_stream_version(&self, thread_id: &str) -> Result<Option<i64>> {
        let stream_name = thread_stream_name(thread_id);

        match self.client.get_last_stream_message(&stream_name, None).await {
            Ok(Some(msg)) => Ok(Some(msg.position)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Convert a MessageDB Message to a ThreadEvent
    ///
    /// This reconstructs the original ThreadEvent from storage.
    fn message_to_thread_event(&self, msg: Message) -> Result<ThreadEvent> {
        // The ThreadEvent is stored in the data field with a "type" discriminator
        // We need to reconstruct it by combining the type with the data
        let mut data_with_type = msg.data;

        // Add the type field from message_type
        if let Value::Object(ref mut map) = data_with_type {
            map.insert("type".to_string(), Value::String(msg.message_type));
        }

        // Deserialize into ThreadEvent
        serde_json::from_value(data_with_type).map_err(|e| {
            Error::ValidationError(format!(
                "Failed to deserialize ThreadEvent from message {}: {}",
                msg.id, e
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::agent::events::*;
    use chrono::Utc;
    use serde_json::json;

    // Unit tests will use mocked MessageDB client
    // Integration tests (below) will use real testcontainers

    #[test]
    fn test_thread_stream_name_format() {
        let name = thread_stream_name("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(name, "thread:v0-550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_event_to_write_message_conversion() {
        let now = Utc::now();
        let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Hello!".to_string(),
            timestamp: now,
        });

        let write_msg = event.to_write_message("thread:v0-123", None);

        assert_eq!(write_msg.stream_name, "thread:v0-123");
        assert_eq!(write_msg.message_type, "UserMessageReceived");
        assert_eq!(write_msg.data["message"], "Hello!");
    }

    #[test]
    fn test_event_with_metadata() {
        let now = Utc::now();
        let event = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 1,
            timestamp: now,
        });

        let metadata = json!({
            "correlation_id": "corr-123",
            "user_id": "user-456"
        });

        let write_msg = event.to_write_message("thread:v0-abc", Some(metadata.clone()));

        assert_eq!(write_msg.metadata.as_ref().unwrap()["correlation_id"], "corr-123");
        assert_eq!(write_msg.metadata.as_ref().unwrap()["user_id"], "user-456");
    }
}
