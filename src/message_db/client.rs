use deadpool_postgres::Pool;

use crate::message_db::{
    connection::MessageDbConfig,
    error::Result,
    operations::{self, CategoryReadOptions, StreamReadOptions},
    transaction::Transaction,
    types::{Message, WriteMessage},
};

/// Main Message DB client
#[derive(Clone)]
pub struct MessageDbClient {
    pool: Pool,
    schema_name: String,
}

impl MessageDbClient {
    /// Create a new Message DB client from configuration
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///
    ///     let client = MessageDbClient::new(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(config: MessageDbConfig) -> Result<Self> {
        let schema_name = config.schema_name.clone();
        let pool = config.build_pool()?;

        // Test the connection
        let _conn = pool.get().await?;

        Ok(Self { pool, schema_name })
    }

    /// Get a reference to the connection pool
    // pub(crate) fn pool(&self) -> &Pool {
    //     &self.pool
    // }

    // /// Get the schema name
    // pub(crate) fn schema_name(&self) -> &str {
    //     &self.schema_name
    // }

    /// Write a message to a stream with optional optimistic concurrency control
    ///
    /// Returns the stream position of the written message.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// use rust2::message_db::types::WriteMessage;
    /// use uuid::Uuid;
    /// use serde_json::json;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let msg = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
    ///         .with_data(json!({ "amount": 50 }));
    ///
    ///     let position = client.write_message(msg).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn write_message(&self, msg: WriteMessage) -> Result<i64> {
        operations::write_message(&self.pool, &self.schema_name, msg).await
    }

    /// Retrieve messages from a single stream
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// use rust2::message_db::operations::StreamReadOptions;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let options = StreamReadOptions::new("account-123")
    ///         .with_batch_size(100);
    ///
    ///     let messages = client.get_stream_messages(options).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_stream_messages(&self, options: StreamReadOptions) -> Result<Vec<Message>> {
        operations::get_stream_messages(&self.pool, &self.schema_name, options).await
    }

    /// Retrieve messages from all streams in a category
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// use rust2::message_db::operations::CategoryReadOptions;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let options = CategoryReadOptions::new("account")
    ///         .with_consumer_group(0, 3);
    ///
    ///     let messages = client.get_category_messages(options).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_category_messages(&self, options: CategoryReadOptions) -> Result<Vec<Message>> {
        operations::get_category_messages(&self.pool, &self.schema_name, options).await
    }

    /// Retrieve the most recent message from a stream
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let last_msg = client.get_last_stream_message("account-123", None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_last_stream_message(
        &self,
        stream_name: &str,
        message_type: Option<&str>,
    ) -> Result<Option<Message>> {
        operations::get_last_stream_message(&self.pool, &self.schema_name, stream_name, message_type).await
    }

    /// Get the current version (position of last message) of a stream
    ///
    /// Returns None if the stream doesn't exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let version = client.stream_version("account-123").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn stream_version(&self, stream_name: &str) -> Result<Option<i64>> {
        operations::stream_version(&self.pool, &self.schema_name, stream_name).await
    }

    /// Begin a new database transaction
    ///
    /// Returns a `Transaction` object that can be used to perform multiple
    /// operations atomically. The transaction must be explicitly committed
    /// or rolled back.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust2::message_db::{MessageDbClient, MessageDbConfig};
    /// use rust2::message_db::types::WriteMessage;
    /// use uuid::Uuid;
    /// use serde_json::json;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = MessageDbConfig::from_connection_string(
    ///         "postgresql://postgres:password@localhost:5432/message_store"
    ///     )?;
    ///     let client = MessageDbClient::new(config).await?;
    ///
    ///     let mut txn = client.begin_transaction().await?;
    ///
    ///     // Write multiple messages atomically
    ///     let msg1 = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
    ///         .with_data(json!({ "amount": 50 }));
    ///     let msg2 = WriteMessage::new(Uuid::new_v4(), "account-456", "Deposited")
    ///         .with_data(json!({ "amount": 50 }));
    ///
    ///     txn.write_message(msg1).await?;
    ///     txn.write_message(msg2).await?;
    ///
    ///     txn.commit().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn begin_transaction(&self) -> Result<Transaction> {
        let conn = self.pool.get().await?;
        Transaction::begin(conn, self.schema_name.clone()).await
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_client_creation() {
        // This is a compile-time test to ensure the API is correct
        // Actual connection testing is done in integration tests
    }
}
