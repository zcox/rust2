use crate::message_db::{
    error::{Error, Result},
    types::Message,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use deadpool_postgres::Pool;
use serde_json::Value;
use tokio_postgres::Row;
use uuid::Uuid;

/// Options for reading stream messages
#[derive(Debug, Clone)]
pub struct StreamReadOptions {
    /// Stream to read from
    pub stream_name: String,

    /// Starting position (inclusive, 0-based)
    pub position: i64,

    /// Maximum messages to retrieve
    pub batch_size: i64,

    /// Optional SQL WHERE condition for filtering
    pub condition: Option<String>,
}

impl StreamReadOptions {
    /// Create new stream read options
    pub fn new(stream_name: impl Into<String>) -> Self {
        Self {
            stream_name: stream_name.into(),
            position: 0,
            batch_size: 1000,
            condition: None,
        }
    }

    /// Set the starting position (builder pattern)
    pub fn with_position(mut self, position: i64) -> Self {
        self.position = position;
        self
    }

    /// Set the batch size (builder pattern)
    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Set the SQL condition (builder pattern)
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

/// Options for reading category messages
#[derive(Debug, Clone)]
pub struct CategoryReadOptions {
    /// Category to read from
    pub category_name: String,

    /// Starting global position (inclusive, 1-based for categories)
    pub position: i64,

    /// Maximum messages to retrieve
    pub batch_size: i64,

    /// Optional correlation category for filtering
    pub correlation: Option<String>,

    /// Consumer group member ID (0-based)
    pub consumer_group_member: Option<i64>,

    /// Total consumer group size
    pub consumer_group_size: Option<i64>,

    /// Optional SQL WHERE condition for filtering
    pub condition: Option<String>,
}

impl CategoryReadOptions {
    /// Create new category read options
    pub fn new(category_name: impl Into<String>) -> Self {
        Self {
            category_name: category_name.into(),
            position: 1,
            batch_size: 1000,
            correlation: None,
            consumer_group_member: None,
            consumer_group_size: None,
            condition: None,
        }
    }

    /// Set the starting position (builder pattern)
    pub fn with_position(mut self, position: i64) -> Self {
        self.position = position;
        self
    }

    /// Set the batch size (builder pattern)
    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size;
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

/// Parse a message row from the database
///
/// This is a public helper function that can be used by transaction code
/// and other internal modules.
pub(crate) fn parse_message_row(row: &Row) -> Result<Message> {
    // Message DB stores UUIDs as text, so we need to parse them
    let id_str: String = row.get("id");
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| Error::DatabaseError(format!("Invalid UUID in database: {}", e)))?;

    // Message DB returns data and metadata as text (varchar), not JSONB
    // We need to parse them ourselves
    let data_str: String = row.get("data");
    let data: Value = serde_json::from_str(&data_str)
        .map_err(|e| Error::DatabaseError(format!("Invalid JSON in data column: {}", e)))?;

    let metadata: Option<Value> = row
        .get::<_, Option<String>>("metadata")
        .and_then(|s| serde_json::from_str(&s).ok());

    // Message DB returns timestamp without timezone, but the message was written in UTC
    let naive_time: NaiveDateTime = row.get("time");
    let time = DateTime::<Utc>::from_naive_utc_and_offset(naive_time, Utc);

    Ok(Message {
        id,
        stream_name: row.get("stream_name"),
        message_type: row.get("type"),
        data,
        metadata,
        position: row.get("position"),
        global_position: row.get("global_position"),
        time,
    })
}

/// Retrieve messages from a single stream
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `schema_name` - Message DB schema name (typically "message_store")
/// * `options` - Stream read options
///
/// # Returns
///
/// Returns a list of messages ordered by position
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
///         .with_position(0)
///         .with_batch_size(100);
///
///     let messages = client.get_stream_messages(options).await?;
///     println!("Retrieved {} messages", messages.len());
///     Ok(())
/// }
/// ```
pub async fn get_stream_messages(
    pool: &Pool,
    schema_name: &str,
    options: StreamReadOptions,
) -> Result<Vec<Message>> {
    let conn = pool.get().await?;

    // Construct the function call SQL
    let sql = format!(
        "SELECT * FROM {}.get_stream_messages($1, $2, $3, $4)",
        schema_name
    );

    // Execute the function call
    let rows = conn
        .query(
            &sql,
            &[
                &options.stream_name,
                &options.position,
                &options.batch_size,
                &options.condition,
            ],
        )
        .await?;

    // Parse the results
    rows.iter().map(parse_message_row).collect()
}

/// Retrieve messages from all streams in a category
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `schema_name` - Message DB schema name (typically "message_store")
/// * `options` - Category read options
///
/// # Returns
///
/// Returns a list of messages ordered by global position
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
///         .with_position(1)
///         .with_batch_size(100)
///         .with_consumer_group(0, 3);
///
///     let messages = client.get_category_messages(options).await?;
///     println!("Retrieved {} messages", messages.len());
///     Ok(())
/// }
/// ```
pub async fn get_category_messages(
    pool: &Pool,
    schema_name: &str,
    options: CategoryReadOptions,
) -> Result<Vec<Message>> {
    let conn = pool.get().await?;

    // Construct the function call SQL
    let sql = format!(
        "SELECT * FROM {}.get_category_messages($1, $2, $3, $4, $5, $6, $7)",
        schema_name
    );

    // Execute the function call
    let rows = conn
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
        .await?;

    // Parse the results
    rows.iter().map(parse_message_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_read_options_builder() {
        let opts = StreamReadOptions::new("account-123")
            .with_position(10)
            .with_batch_size(50)
            .with_condition("type = 'Withdrawn'");

        assert_eq!(opts.stream_name, "account-123");
        assert_eq!(opts.position, 10);
        assert_eq!(opts.batch_size, 50);
        assert_eq!(opts.condition, Some("type = 'Withdrawn'".to_string()));
    }

    #[test]
    fn test_category_read_options_builder() {
        let opts = CategoryReadOptions::new("account")
            .with_position(100)
            .with_batch_size(25)
            .with_correlation("withdrawal-cmd")
            .with_consumer_group(0, 3)
            .with_condition("type IN ('Deposited', 'Withdrawn')");

        assert_eq!(opts.category_name, "account");
        assert_eq!(opts.position, 100);
        assert_eq!(opts.batch_size, 25);
        assert_eq!(opts.correlation, Some("withdrawal-cmd".to_string()));
        assert_eq!(opts.consumer_group_member, Some(0));
        assert_eq!(opts.consumer_group_size, Some(3));
        assert_eq!(opts.condition, Some("type IN ('Deposited', 'Withdrawn')".to_string()));
    }
}
