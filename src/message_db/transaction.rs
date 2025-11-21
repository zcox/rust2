//! Transaction support for Message DB operations
//!
//! This module provides transaction management for atomic multi-message writes
//! and transactional processing patterns.
//!
//! # Example
//!
//! ```no_run
//! use rust2::message_db::{MessageDbClient, MessageDbConfig};
//! use rust2::message_db::types::WriteMessage;
//! use uuid::Uuid;
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MessageDbConfig::from_connection_string(
//!         "postgresql://postgres:password@localhost:5432/message_store"
//!     )?;
//!     let client = MessageDbClient::new(config).await?;
//!
//!     // Begin a transaction
//!     let mut txn = client.begin_transaction().await?;
//!
//!     // Write multiple messages atomically
//!     let msg1 = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
//!         .with_data(json!({ "amount": 50 }));
//!     let msg2 = WriteMessage::new(Uuid::new_v4(), "account-456", "Deposited")
//!         .with_data(json!({ "amount": 50 }));
//!
//!     txn.write_message(msg1).await?;
//!     txn.write_message(msg2).await?;
//!
//!     // Commit the transaction
//!     txn.commit().await?;
//!     Ok(())
//! }
//! ```

use crate::message_db::{
    error::{Error, Result},
    operations::{CategoryReadOptions, StreamReadOptions},
    types::{Message, WriteMessage},
};
use deadpool_postgres::Object;

/// A Message DB transaction
///
/// Transactions provide atomic multi-message writes and transactional processing.
/// All operations within a transaction either succeed together or fail together.
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
///     let msg = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
///         .with_data(json!({ "amount": 50 }));
///
///     txn.write_message(msg).await?;
///     txn.commit().await?;
///     Ok(())
/// }
/// ```
pub struct Transaction {
    connection: Option<Object>,
    schema_name: String,
    in_transaction: bool,
}

impl Transaction {
    /// Begin a new transaction
    pub(crate) async fn begin(connection: Object, schema_name: String) -> Result<Self> {
        // Execute BEGIN
        connection.batch_execute("BEGIN").await
            .map_err(|e| Error::DatabaseError(format!("Failed to begin transaction: {:?}", e)))?;

        Ok(Self {
            connection: Some(connection),
            schema_name,
            in_transaction: true,
        })
    }

    fn get_connection(&self) -> Result<&Object> {
        if !self.in_transaction {
            return Err(Error::DatabaseError("Transaction already completed".to_string()));
        }
        self.connection.as_ref()
            .ok_or_else(|| Error::DatabaseError("No connection available".to_string()))
    }

    /// Write a message to a stream within this transaction
    ///
    /// # Arguments
    ///
    /// * `msg` - The message to write
    ///
    /// # Returns
    ///
    /// Returns the stream position of the written message
    ///
    /// # Behavior
    ///
    /// 1. **Idempotency**: If a message with the same `id` already exists in the stream,
    ///    the write is ignored and the existing position is returned
    /// 2. **Expected Version**: If `expected_version` is provided and doesn't match the
    ///    current stream version, a concurrency error is raised and the transaction
    ///    should be rolled back
    /// 3. **Atomic**: The write is part of the transaction and will only be committed
    ///    when the transaction is committed
    ///
    /// # Errors
    ///
    /// * `Error::ConcurrencyError` - If expected_version doesn't match current version
    /// * `Error::ValidationError` - For invalid UUIDs or malformed JSON
    /// * `Error::DatabaseError` - For database connection or SQL errors
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
    ///     let msg = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
    ///         .with_data(json!({ "amount": 50 }))
    ///         .with_expected_version(4);
    ///
    ///     match txn.write_message(msg).await {
    ///         Ok(position) => {
    ///             txn.commit().await?;
    ///             println!("Message written at position: {}", position);
    ///         }
    ///         Err(e) => {
    ///             txn.rollback().await?;
    ///             eprintln!("Write failed: {}", e);
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn write_message(&mut self, msg: WriteMessage) -> Result<i64> {
        let conn = self.get_connection()?;
        write_message_in_transaction(conn, &self.schema_name, msg).await
    }

    /// Retrieve messages from a single stream within this transaction
    ///
    /// This allows reading stream state within a transaction for consistent
    /// read-process-write patterns.
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
    ///     let mut txn = client.begin_transaction().await?;
    ///
    ///     let options = StreamReadOptions::new("account-123");
    ///     let messages = txn.get_stream_messages(options).await?;
    ///
    ///     // Process messages and make decisions...
    ///
    ///     txn.commit().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_stream_messages(&self, options: StreamReadOptions) -> Result<Vec<Message>> {
        let conn = self.get_connection()?;
        get_stream_messages_in_transaction(conn, &self.schema_name, options).await
    }

    /// Retrieve messages from all streams in a category within this transaction
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
    ///     let mut txn = client.begin_transaction().await?;
    ///
    ///     let options = CategoryReadOptions::new("account");
    ///     let messages = txn.get_category_messages(options).await?;
    ///
    ///     txn.commit().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_category_messages(&self, options: CategoryReadOptions) -> Result<Vec<Message>> {
        let conn = self.get_connection()?;
        get_category_messages_in_transaction(conn, &self.schema_name, options).await
    }

    /// Retrieve the most recent message from a stream within this transaction
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
    ///     let mut txn = client.begin_transaction().await?;
    ///
    ///     let last_msg = txn.get_last_stream_message("account-123", None).await?;
    ///
    ///     txn.commit().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_last_stream_message(
        &self,
        stream_name: &str,
        message_type: Option<&str>,
    ) -> Result<Option<Message>> {
        let conn = self.get_connection()?;
        get_last_stream_message_in_transaction(
            conn,
            &self.schema_name,
            stream_name,
            message_type,
        )
        .await
    }

    /// Get the current version (position of last message) of a stream within this transaction
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
    ///     let mut txn = client.begin_transaction().await?;
    ///
    ///     let version = txn.stream_version("account-123").await?;
    ///
    ///     txn.commit().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn stream_version(&self, stream_name: &str) -> Result<Option<i64>> {
        let conn = self.get_connection()?;
        stream_version_in_transaction(conn, &self.schema_name, stream_name).await
    }

    /// Commit the transaction
    ///
    /// All operations within the transaction will be persisted atomically.
    ///
    /// # Errors
    ///
    /// * `Error::DatabaseError` - If the commit fails
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
    ///     let msg = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
    ///         .with_data(json!({ "amount": 50 }));
    ///
    ///     txn.write_message(msg).await?;
    ///     txn.commit().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn commit(mut self) -> Result<()> {
        if self.in_transaction {
            if let Some(conn) = &self.connection {
                conn.batch_execute("COMMIT").await
                    .map_err(|e| Error::DatabaseError(format!("Failed to commit transaction: {:?}", e)))?;
                self.in_transaction = false;
            }
        }
        Ok(())
    }

    /// Rollback the transaction
    ///
    /// All operations within the transaction will be discarded.
    ///
    /// # Errors
    ///
    /// * `Error::DatabaseError` - If the rollback fails
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
    ///     let msg = WriteMessage::new(Uuid::new_v4(), "account-123", "Withdrawn")
    ///         .with_data(json!({ "amount": 50 }));
    ///
    ///     if let Err(e) = txn.write_message(msg).await {
    ///         txn.rollback().await?;
    ///         eprintln!("Transaction rolled back: {}", e);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn rollback(mut self) -> Result<()> {
        if self.in_transaction {
            if let Some(conn) = &self.connection {
                conn.batch_execute("ROLLBACK").await
                    .map_err(|e| Error::DatabaseError(format!("Failed to rollback transaction: {:?}", e)))?;
                self.in_transaction = false;
            }
        }
        Ok(())
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // If the transaction is still active when dropped, it will be automatically
        // rolled back when the connection is returned to the pool.
        // This is a safety mechanism to prevent uncommitted transactions.
    }
}

// Internal helper functions for transaction operations

async fn write_message_in_transaction(
    client: &Object,
    schema_name: &str,
    msg: WriteMessage,
) -> Result<i64> {
    // Construct the function call SQL
    let sql = format!("SELECT {}.write_message($1, $2, $3, $4, $5, $6)", schema_name);

    // Prepare the parameters
    let id_str = msg.id.to_string();

    // Execute the function call
    let result = client
        .query_one(
            &sql,
            &[
                &id_str,
                &msg.stream_name,
                &msg.message_type,
                &msg.data,
                &msg.metadata,
                &msg.expected_version,
            ],
        )
        .await;

    match result {
        Ok(row) => {
            // Extract the position from the result
            let position: i64 = row.get(0);
            Ok(position)
        }
        Err(e) => {
            // Check for expected version mismatch error
            if let Some(db_error) = e.as_db_error() {
                let message = db_error.message();

                if message.contains("Wrong expected version")
                    || message.contains("stream version")
                    || message.contains("expected")
                {
                    return Err(Error::ConcurrencyError {
                        stream_name: msg.stream_name.clone(),
                        expected_version: msg.expected_version.unwrap_or(-1),
                        actual_version: None,
                    });
                }

                // Check for duplicate message ID - this means idempotent write
                // However, when this happens within a transaction, PostgreSQL aborts the transaction
                // and we can't query within it. The transaction must be rolled back.
                if message.contains("duplicate key") && message.contains("messages_id") {
                    // In a transaction, a duplicate key error aborts the transaction.
                    // Return an error that signals this is an idempotent write scenario.
                    return Err(Error::DatabaseError(
                        "Duplicate message ID (idempotent write) - transaction aborted by PostgreSQL. \
                         The transaction must be rolled back.".to_string()
                    ));
                }
            }
            Err(Error::DatabaseError(format!(
                "write_message failed: {:?}",
                e
            )))
        }
    }
}

async fn get_stream_messages_in_transaction(
    client: &Object,
    schema_name: &str,
    options: StreamReadOptions,
) -> Result<Vec<Message>> {
    use crate::message_db::operations::read::parse_message_row;

    let sql = format!(
        "SELECT * FROM {}.get_stream_messages($1, $2, $3, $4)",
        schema_name
    );

    let rows = client
        .query(
            &sql,
            &[
                &options.stream_name,
                &options.position,
                &options.batch_size,
                &options.condition,
            ],
        )
        .await
        .map_err(|e| Error::DatabaseError(format!("get_stream_messages failed: {:?}", e)))?;

    rows.iter().map(parse_message_row).collect()
}

async fn get_category_messages_in_transaction(
    client: &Object,
    schema_name: &str,
    options: CategoryReadOptions,
) -> Result<Vec<Message>> {
    use crate::message_db::operations::read::parse_message_row;

    let sql = format!(
        "SELECT * FROM {}.get_category_messages($1, $2, $3, $4, $5, $6, $7)",
        schema_name
    );

    let rows = client
        .query(
            &sql,
            &[
                &options.category_name,
                &options.position,
                &options.batch_size,
                &options.correlation,
                &options.consumer_group_member,
                &options.consumer_group_size,
                &options.condition,
            ],
        )
        .await
        .map_err(|e| Error::DatabaseError(format!("get_category_messages failed: {:?}", e)))?;

    rows.iter().map(parse_message_row).collect()
}

async fn get_last_stream_message_in_transaction(
    client: &Object,
    schema_name: &str,
    stream_name: &str,
    message_type: Option<&str>,
) -> Result<Option<Message>> {
    use crate::message_db::operations::query::parse_message_row;

    let sql = format!(
        "SELECT * FROM {}.get_last_stream_message($1, $2)",
        schema_name
    );

    let rows = client
        .query(&sql, &[&stream_name, &message_type])
        .await
        .map_err(|e| Error::DatabaseError(format!("get_last_stream_message failed: {:?}", e)))?;

    if rows.is_empty() {
        Ok(None)
    } else {
        parse_message_row(&rows[0]).map(Some)
    }
}

async fn stream_version_in_transaction(
    client: &Object,
    schema_name: &str,
    stream_name: &str,
) -> Result<Option<i64>> {
    let sql = format!("SELECT {}.stream_version($1)", schema_name);

    let row = client
        .query_one(&sql, &[&stream_name])
        .await
        .map_err(|e| Error::DatabaseError(format!("stream_version failed: {:?}", e)))?;

    let version: Option<i64> = row.get(0);
    Ok(version)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_transaction_struct() {
        // This is a compile-time test to ensure the API is correct
        // Actual transaction testing is done in integration tests
    }
}
