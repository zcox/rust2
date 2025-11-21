use crate::message_db::{
    error::{Error, Result},
    types::WriteMessage,
};
use deadpool_postgres::Pool;

/// Write a message to a stream with optional optimistic concurrency control
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `schema_name` - Message DB schema name (typically "message_store")
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
///    current stream version, a concurrency error is raised
/// 3. **Atomic**: The write operation is atomic
/// 4. **Timestamp**: The database sets the `time` field automatically
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
///     let msg = WriteMessage::new(
///         Uuid::new_v4(),
///         "account-123",
///         "Withdrawn"
///     )
///     .with_data(json!({ "amount": 50, "currency": "USD" }))
///     .with_metadata(json!({ "correlation_id": "xyz-789" }))
///     .with_expected_version(4);
///
///     let position = client.write_message(msg).await?;
///     println!("Message written at position: {}", position);
///     Ok(())
/// }
/// ```
pub async fn write_message(
    pool: &Pool,
    schema_name: &str,
    msg: WriteMessage,
) -> Result<i64> {
    let conn = pool.get().await?;

    // Construct the function call SQL
    let sql = format!(
        "SELECT {}.write_message($1, $2, $3, $4, $5, $6)",
        schema_name
    );

    // Prepare the parameters
    let id_str = msg.id.to_string();

    // Execute the function call
    let result = conn
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
                    || message.contains("expected") {
                    return Err(Error::ConcurrencyError {
                        stream_name: msg.stream_name.clone(),
                        expected_version: msg.expected_version.unwrap_or(-1),
                        actual_version: None,
                    });
                }

                // Check for duplicate message ID - this means idempotent write
                // Some Message DB versions don't handle idempotency internally, so we need to
                // query for the existing message's position
                if message.contains("duplicate key") && message.contains("messages_id") {
                    // Query for the existing message to get its position
                    // Note: id column is UUID type in messages table
                    let query_sql = format!(
                        "SELECT position FROM {}.messages WHERE id = $1 AND stream_name = $2",
                        schema_name
                    );

                    let existing_row = conn
                        .query_one(&query_sql, &[&msg.id, &msg.stream_name])
                        .await
                        .map_err(|e| Error::DatabaseError(format!("Failed to query existing message position: {:?}", e)))?;

                    let position: i64 = existing_row.get(0);
                    return Ok(position);
                }
            }
            // Include more details in error
            Err(Error::DatabaseError(format!("write_message failed: {:?}", e)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_write_message_struct() {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            "account-123",
            "Withdrawn"
        )
        .with_data(json!({ "amount": 50 }))
        .with_expected_version(4);

        assert_eq!(msg.stream_name, "account-123");
        assert_eq!(msg.message_type, "Withdrawn");
        assert_eq!(msg.expected_version, Some(4));
    }
}
