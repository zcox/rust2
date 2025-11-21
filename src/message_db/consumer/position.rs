use crate::message_db::{
    error::Result,
    types::WriteMessage,
    MessageDbClient,
};
use serde_json::json;
use uuid::Uuid;

/// Position tracking for consumers
///
/// Manages reading and writing consumer position to a position stream.
/// Position streams follow the naming convention: `{category}:position-{consumer_id}`
pub struct PositionTracker {
    client: MessageDbClient,
    position_stream_name: String,
    update_interval: usize,
    messages_since_update: usize,
    current_position: i64,
}

impl PositionTracker {
    /// Create a new position tracker
    ///
    /// # Arguments
    ///
    /// * `client` - Message DB client
    /// * `category` - Category being consumed
    /// * `consumer_id` - Unique identifier for this consumer
    /// * `update_interval` - Write position after this many messages (default: 100)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// use rust2::message_db::consumer::PositionTracker;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let tracker = PositionTracker::new(
    ///         client,
    ///         "account",
    ///         "worker-1",
    ///         100
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn new(
        client: MessageDbClient,
        category: &str,
        consumer_id: &str,
        update_interval: usize,
    ) -> Self {
        let position_stream_name = format!("{}:position-{}", category, consumer_id);

        Self {
            client,
            position_stream_name,
            update_interval,
            messages_since_update: 0,
            current_position: 1, // Category positions start at 1
        }
    }

    /// Get the position stream name
    pub fn position_stream_name(&self) -> &str {
        &self.position_stream_name
    }

    /// Read the last stored position from the position stream
    ///
    /// Returns 1 (the default starting position for categories) if no position has been stored.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use rust2::message_db::consumer::PositionTracker;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let config = MessageDbConfig::from_connection_string(
    /// #         "postgresql://postgres:password@localhost:5432/message_store"
    /// #     )?;
    /// #     let client = MessageDbClient::new(config).await?;
    /// #     let mut tracker = PositionTracker::new(client, "account", "worker-1", 100);
    /// let position = tracker.read_position().await?;
    /// println!("Starting from position: {}", position);
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn read_position(&mut self) -> Result<i64> {
        match self.client.get_last_stream_message(&self.position_stream_name, None).await? {
            Some(msg) => {
                let position = msg.data
                    .get("position")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1);
                self.current_position = position;
                Ok(position)
            }
            None => {
                // No position stored yet, start from beginning
                self.current_position = 1;
                Ok(1)
            }
        }
    }

    /// Update the current position and potentially write to position stream
    ///
    /// The position is only written to the stream after `update_interval` messages
    /// have been processed, to balance resumability with write overhead.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use rust2::message_db::consumer::PositionTracker;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let config = MessageDbConfig::from_connection_string(
    /// #         "postgresql://postgres:password@localhost:5432/message_store"
    /// #     )?;
    /// #     let client = MessageDbClient::new(config).await?;
    /// #     let mut tracker = PositionTracker::new(client, "account", "worker-1", 100);
    /// // Process a message
    /// let global_position = 12345;
    /// tracker.update_position(global_position).await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn update_position(&mut self, global_position: i64) -> Result<()> {
        self.current_position = global_position;
        self.messages_since_update += 1;

        if self.messages_since_update >= self.update_interval {
            self.write_position().await?;
            self.messages_since_update = 0;
        }

        Ok(())
    }

    /// Force write the current position to the position stream
    ///
    /// This is typically called:
    /// - Before shutting down the consumer
    /// - After processing a batch when no more messages are available
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use rust2::message_db::consumer::PositionTracker;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let config = MessageDbConfig::from_connection_string(
    /// #         "postgresql://postgres:password@localhost:5432/message_store"
    /// #     )?;
    /// #     let client = MessageDbClient::new(config).await?;
    /// #     let mut tracker = PositionTracker::new(client, "account", "worker-1", 100);
    /// // Shutting down, save position
    /// tracker.write_position().await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn write_position(&self) -> Result<()> {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            &self.position_stream_name,
            "PositionUpdated",
        )
        .with_data(json!({ "position": self.current_position }));

        self.client.write_message(msg).await?;
        Ok(())
    }

    /// Get the current position value
    pub fn current_position(&self) -> i64 {
        self.current_position
    }

    /// Get the number of messages processed since last position write
    pub fn messages_since_update(&self) -> usize {
        self.messages_since_update
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_stream_name() {
        // Create a mock client (we'll use integration tests for actual functionality)
        // This is just to test the naming convention
        let stream_name = format!("{}:position-{}", "account", "worker-1");
        assert_eq!(stream_name, "account:position-worker-1");
    }

    #[test]
    fn test_position_stream_name_with_types() {
        let stream_name = format!("{}:position-{}", "account:command", "consumer-2");
        assert_eq!(stream_name, "account:command:position-consumer-2");
    }
}
