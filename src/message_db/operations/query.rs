use crate::message_db::{
    error::Result,
    types::Message,
};
use deadpool_postgres::Pool;

pub(crate) use super::read::parse_message_row;

/// Retrieve the most recent message from a stream
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `schema_name` - Message DB schema name (typically "message_store")
/// * `stream_name` - Stream to read from
/// * `message_type` - Optional message type filter
///
/// # Returns
///
/// Returns the last message, or None if the stream is empty or no message matches the type
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
///     // Get last message of any type
///     let last_msg = client.get_last_stream_message("account-123", None).await?;
///
///     // Get last message of specific type
///     let last_withdrawal = client.get_last_stream_message(
///         "account-123",
///         Some("Withdrawn")
///     ).await?;
///
///     Ok(())
/// }
/// ```
pub async fn get_last_stream_message(
    pool: &Pool,
    schema_name: &str,
    stream_name: &str,
    message_type: Option<&str>,
) -> Result<Option<Message>> {
    let conn = pool.get().await?;

    // Construct the function call SQL
    let sql = format!(
        "SELECT * FROM {}.get_last_stream_message($1, $2)",
        schema_name
    );

    // Execute the function call
    let rows = conn
        .query(
            &sql,
            &[&stream_name, &message_type],
        )
        .await?;

    // Parse the result
    if rows.is_empty() {
        Ok(None)
    } else {
        parse_message_row(&rows[0]).map(Some)
    }
}

/// Get the current version (position of last message) of a stream
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `schema_name` - Message DB schema name (typically "message_store")
/// * `stream_name` - Stream to check
///
/// # Returns
///
/// Returns the position of the last message, or None if the stream doesn't exist
///
/// For a stream with one message, returns 0.
/// For a stream with n messages, returns n-1.
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
///     match client.stream_version("account-123").await? {
///         Some(version) => println!("Stream version: {}", version),
///         None => println!("Stream does not exist"),
///     }
///
///     Ok(())
/// }
/// ```
pub async fn stream_version(
    pool: &Pool,
    schema_name: &str,
    stream_name: &str,
) -> Result<Option<i64>> {
    let conn = pool.get().await?;

    // Construct the function call SQL
    let sql = format!(
        "SELECT {}.stream_version($1)",
        schema_name
    );

    // Execute the function call
    let row = conn
        .query_one(
            &sql,
            &[&stream_name],
        )
        .await?;

    // Extract the version
    let version: Option<i64> = row.get(0);
    Ok(version)
}

#[cfg(test)]
mod tests {
    // Unit tests would go here, but these functions are primarily integration-tested
    #[test]
    fn test_placeholder() {
        // Placeholder test
    }
}
