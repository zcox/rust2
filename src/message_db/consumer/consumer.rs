use crate::message_db::{
    consumer::PositionTracker,
    error::Result,
    operations::CategoryReadOptions,
    types::Message,
    MessageDbClient,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

/// Type alias for message handler functions
///
/// Handlers are async functions that take a message and return a Result.
pub type MessageHandler = Arc<
    dyn Fn(Message) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync,
>;

/// Configuration for a consumer
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Category to consume from
    pub category: String,

    /// Unique identifier for this consumer
    pub consumer_id: String,

    /// Maximum messages to retrieve per batch
    pub batch_size: i64,

    /// Wait time when no messages are available (milliseconds)
    pub polling_interval_ms: u64,

    /// Write position after this many messages
    pub position_update_interval: usize,

    /// Optional correlation category for filtering
    pub correlation: Option<String>,

    /// Consumer group member ID (0-based)
    pub consumer_group_member: Option<i64>,

    /// Total consumer group size
    pub consumer_group_size: Option<i64>,

    /// Optional SQL WHERE condition for filtering
    pub condition: Option<String>,
}

impl ConsumerConfig {
    /// Create a new consumer configuration
    ///
    /// # Arguments
    ///
    /// * `category` - Category to consume from
    /// * `consumer_id` - Unique identifier for this consumer
    ///
    /// # Example
    ///
    /// ```
    /// use rust2::message_db::consumer::ConsumerConfig;
    ///
    /// let config = ConsumerConfig::new("account", "worker-1")
    ///     .with_batch_size(50)
    ///     .with_polling_interval_ms(200)
    ///     .with_consumer_group(0, 3);
    /// ```
    pub fn new(category: impl Into<String>, consumer_id: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            consumer_id: consumer_id.into(),
            batch_size: 10,
            polling_interval_ms: 100,
            position_update_interval: 100,
            correlation: None,
            consumer_group_member: None,
            consumer_group_size: None,
            condition: None,
        }
    }

    /// Set the batch size (builder pattern)
    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Set the polling interval in milliseconds (builder pattern)
    pub fn with_polling_interval_ms(mut self, interval_ms: u64) -> Self {
        self.polling_interval_ms = interval_ms;
        self
    }

    /// Set the position update interval (builder pattern)
    pub fn with_position_update_interval(mut self, interval: usize) -> Self {
        self.position_update_interval = interval;
        self
    }

    /// Set the correlation category (builder pattern)
    pub fn with_correlation(mut self, correlation: impl Into<String>) -> Self {
        self.correlation = Some(correlation.into());
        self
    }

    /// Set the consumer group parameters (builder pattern)
    pub fn with_consumer_group(mut self, member: i64, size: i64) -> Self {
        self.consumer_group_member = Some(member);
        self.consumer_group_size = Some(size);
        self
    }

    /// Set the SQL condition (builder pattern)
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

/// Consumer for processing messages from a category
///
/// Implements the consumer pattern for Message DB:
/// 1. Continuously polls for new messages from a category
/// 2. Dispatches messages to registered handlers based on message type
/// 3. Tracks and persists position for resumability
/// 4. Supports consumer groups for horizontal scaling
///
/// # Example
///
/// ```no_run
/// use rust2::message_db::{MessageDbClient, MessageDbConfig};
/// use rust2::message_db::consumer::{Consumer, ConsumerConfig};
/// use rust2::message_db::types::Message;
/// use rust2::message_db::Result;
///
/// async fn handle_withdrawn(msg: Message) -> Result<()> {
///     println!("Processing withdrawal: {:?}", msg.data);
///     Ok(())
/// }
///
/// async fn handle_deposited(msg: Message) -> Result<()> {
///     println!("Processing deposit: {:?}", msg.data);
///     Ok(())
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let config = MessageDbConfig::from_connection_string(
///         "postgresql://postgres:password@localhost:5432/message_store"
///     )?;
///     let client = MessageDbClient::new(config).await?;
///
///     let consumer_config = ConsumerConfig::new("account", "worker-1")
///         .with_batch_size(10);
///
///     let mut consumer = Consumer::new(client, consumer_config).await?;
///
///     // Register message handlers
///     consumer.on("Withdrawn", |msg| Box::pin(async move {
///         handle_withdrawn(msg).await
///     }));
///
///     consumer.on("Deposited", |msg| Box::pin(async move {
///         handle_deposited(msg).await
///     }));
///
///     // Start consuming (runs until error or cancellation)
///     // consumer.start().await?;
///
///     Ok(())
/// }
/// ```
pub struct Consumer {
    client: MessageDbClient,
    config: ConsumerConfig,
    position_tracker: PositionTracker,
    handlers: HashMap<String, MessageHandler>,
}

impl Consumer {
    /// Create a new consumer
    ///
    /// The consumer will automatically read its last position from the position stream.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// use rust2::message_db::consumer::{Consumer, ConsumerConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let consumer_config = ConsumerConfig::new("account", "worker-1");
    ///     let mut consumer = Consumer::new(client, consumer_config).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(client: MessageDbClient, config: ConsumerConfig) -> Result<Self> {
        let mut position_tracker = PositionTracker::new(
            client.clone(),
            &config.category,
            &config.consumer_id,
            config.position_update_interval,
        );

        // Read the last position
        position_tracker.read_position().await?;

        Ok(Self {
            client,
            config,
            position_tracker,
            handlers: HashMap::new(),
        })
    }

    /// Register a message handler for a specific message type
    ///
    /// When a message of the specified type is received, the handler will be called.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use rust2::message_db::consumer::{Consumer, ConsumerConfig};
    /// # use rust2::message_db::types::Message;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let config = MessageDbConfig::from_connection_string(
    /// #         "postgresql://postgres:password@localhost:5432/message_store"
    /// #     )?;
    /// #     let client = MessageDbClient::new(config).await?;
    /// #     let consumer_config = ConsumerConfig::new("account", "worker-1");
    /// #     let mut consumer = Consumer::new(client, consumer_config).await?;
    /// consumer.on("Withdrawn", |msg| Box::pin(async move {
    ///     println!("Amount: {}", msg.data["amount"]);
    ///     Ok(())
    /// }));
    /// #     Ok(())
    /// # }
    /// ```
    pub fn on<F>(&mut self, message_type: &str, handler: F)
    where
        F: Fn(Message) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static,
    {
        self.handlers.insert(message_type.to_string(), Arc::new(handler));
    }

    /// Start consuming messages
    ///
    /// This method runs indefinitely, polling for new messages and dispatching them
    /// to registered handlers. It will only return if:
    /// - An error occurs
    /// - The task is cancelled (via tokio cancellation)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use rust2::message_db::consumer::{Consumer, ConsumerConfig};
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let config = MessageDbConfig::from_connection_string(
    /// #         "postgresql://postgres:password@localhost:5432/message_store"
    /// #     )?;
    /// #     let client = MessageDbClient::new(config).await?;
    /// #     let consumer_config = ConsumerConfig::new("account", "worker-1");
    /// #     let mut consumer = Consumer::new(client, consumer_config).await?;
    /// // Register handlers...
    /// consumer.on("Withdrawn", |msg| Box::pin(async move { Ok(()) }));
    ///
    /// // Start consuming (blocks until error or cancellation)
    /// // consumer.start().await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn start(&mut self) -> Result<()> {
        loop {
            // Poll for messages
            let had_messages = self.poll_once().await?;

            // If no messages, wait before polling again
            if !had_messages {
                time::sleep(Duration::from_millis(self.config.polling_interval_ms)).await;
            }
        }
    }

    /// Poll for messages once and process them
    ///
    /// Returns true if messages were processed, false if the batch was empty.
    ///
    /// This method is useful for:
    /// - Testing
    /// - Custom polling loops
    /// - Processing a single batch
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// # use rust2::message_db::consumer::{Consumer, ConsumerConfig};
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let config = MessageDbConfig::from_connection_string(
    /// #         "postgresql://postgres:password@localhost:5432/message_store"
    /// #     )?;
    /// #     let client = MessageDbClient::new(config).await?;
    /// #     let consumer_config = ConsumerConfig::new("account", "worker-1");
    /// #     let mut consumer = Consumer::new(client, consumer_config).await?;
    /// // Poll once
    /// let had_messages = consumer.poll_once().await?;
    /// if had_messages {
    ///     println!("Processed some messages");
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn poll_once(&mut self) -> Result<bool> {
        // Build read options
        let mut options = CategoryReadOptions::new(&self.config.category)
            .with_position(self.position_tracker.current_position())
            .with_batch_size(self.config.batch_size);

        if let Some(ref correlation) = self.config.correlation {
            options = options.with_correlation(correlation);
        }

        if let (Some(member), Some(size)) = (self.config.consumer_group_member, self.config.consumer_group_size) {
            options = options.with_consumer_group(member, size);
        }

        if let Some(ref condition) = self.config.condition {
            options = options.with_condition(condition);
        }

        // Fetch messages
        let messages = self.client.get_category_messages(options).await?;
        let had_messages = !messages.is_empty();

        // Process each message
        for message in messages {
            self.dispatch_message(message).await?;
        }

        // Write position if batch was empty (good checkpoint)
        if !had_messages && self.position_tracker.messages_since_update() > 0 {
            self.position_tracker.write_position().await?;
        }

        Ok(had_messages)
    }

    /// Dispatch a message to its handler
    async fn dispatch_message(&mut self, message: Message) -> Result<()> {
        let global_position = message.global_position;

        // Call the handler if registered
        if let Some(handler) = self.handlers.get(&message.message_type) {
            let handler = Arc::clone(handler);
            handler(message).await?;
        }

        // Update position to the next position to read (global_position + 1)
        // This is because get_category_messages reads from position inclusive
        self.position_tracker.update_position(global_position + 1).await?;

        Ok(())
    }

    /// Get the current position
    pub fn current_position(&self) -> i64 {
        self.position_tracker.current_position()
    }

    /// Get the position stream name
    pub fn position_stream_name(&self) -> &str {
        self.position_tracker.position_stream_name()
    }

    /// Force write the current position
    ///
    /// Useful before shutting down the consumer.
    pub async fn flush_position(&self) -> Result<()> {
        self.position_tracker.write_position().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consumer_config_builder() {
        let config = ConsumerConfig::new("account", "worker-1")
            .with_batch_size(50)
            .with_polling_interval_ms(200)
            .with_position_update_interval(150)
            .with_correlation("withdrawal-cmd")
            .with_consumer_group(0, 3)
            .with_condition("type = 'Withdrawn'");

        assert_eq!(config.category, "account");
        assert_eq!(config.consumer_id, "worker-1");
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.polling_interval_ms, 200);
        assert_eq!(config.position_update_interval, 150);
        assert_eq!(config.correlation, Some("withdrawal-cmd".to_string()));
        assert_eq!(config.consumer_group_member, Some(0));
        assert_eq!(config.consumer_group_size, Some(3));
        assert_eq!(config.condition, Some("type = 'Withdrawn'".to_string()));
    }
}
