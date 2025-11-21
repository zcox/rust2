/// Consumer module for implementing the consumer pattern
///
/// This module provides:
/// - `Consumer`: Main consumer for polling and processing messages
/// - `ConsumerConfig`: Configuration for consumers
/// - `PositionTracker`: Position tracking for resumability
///
/// # Consumer Pattern
///
/// The consumer pattern continuously reads and processes messages from a category stream:
///
/// 1. **Read Messages**: Poll category for new messages from last position
/// 2. **Dispatch**: Route messages to handlers based on message type
/// 3. **Track Position**: Maintain last processed global position
/// 4. **Persist**: Write position periodically for resumability
/// 5. **Repeat**: Continue polling for new messages
///
/// # Example
///
/// ```no_run
/// use rust2::message_db::{MessageDbClient, MessageDbConfig};
/// use rust2::message_db::consumer::{Consumer, ConsumerConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create client
///     let config = MessageDbConfig::from_connection_string(
///         "postgresql://postgres:password@localhost:5432/message_store"
///     )?;
///     let client = MessageDbClient::new(config).await?;
///
///     // Configure consumer
///     let consumer_config = ConsumerConfig::new("account", "worker-1")
///         .with_batch_size(10)
///         .with_polling_interval_ms(100);
///
///     // Create consumer
///     let mut consumer = Consumer::new(client, consumer_config).await?;
///
///     // Register handlers
///     consumer.on("Withdrawn", |msg| Box::pin(async move {
///         println!("Processing withdrawal: amount={}", msg.data["amount"]);
///         Ok(())
///     }));
///
///     consumer.on("Deposited", |msg| Box::pin(async move {
///         println!("Processing deposit: amount={}", msg.data["amount"]);
///         Ok(())
///     }));
///
///     // Start consuming (runs indefinitely)
///     // consumer.start().await?;
///
///     Ok(())
/// }
/// ```
///
/// # Consumer Groups
///
/// For horizontal scaling, use consumer groups to distribute messages across instances:
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
/// // 3 worker instances, each with unique member ID (0, 1, 2)
/// let consumer_config = ConsumerConfig::new("account", "worker-0")
///     .with_consumer_group(0, 3);  // Member 0 of 3
///
/// let mut consumer = Consumer::new(client, consumer_config).await?;
/// // Register handlers and start...
/// #     Ok(())
/// # }
/// ```
///
/// # Position Tracking
///
/// Position is automatically tracked and persisted:
///
/// - Position stream: `{category}:position-{consumer_id}`
/// - Updated every N messages (configurable)
/// - Allows resuming from last position on restart
/// - Force flush with `consumer.flush_position()`

pub mod consumer;
pub mod position;

pub use consumer::{Consumer, ConsumerConfig, MessageHandler};
pub use position::PositionTracker;
