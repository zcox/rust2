use std::fmt;

/// Result type for Message DB operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for Message DB client operations
#[derive(Debug)]
pub enum Error {
    /// Concurrency error - expected version mismatch
    ConcurrencyError {
        stream_name: String,
        expected_version: i64,
        actual_version: Option<i64>,
    },

    /// Validation error - invalid input data
    ValidationError(String),

    /// Connection error - database unreachable or authentication failure
    ConnectionError(String),

    /// Not found error - stream or message doesn't exist
    NotFoundError(String),

    /// Database error - SQL errors, constraint violations
    DatabaseError(String),

    /// Pool error - connection pool issues
    PoolError(String),

    /// Transaction error - transaction-specific errors
    TransactionError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ConcurrencyError {
                stream_name,
                expected_version,
                actual_version,
            } => write!(
                f,
                "Concurrency error on stream '{}': expected version {}, actual version {:?}",
                stream_name, expected_version, actual_version
            ),
            Error::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Error::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            Error::NotFoundError(msg) => write!(f, "Not found: {}", msg),
            Error::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Error::PoolError(msg) => write!(f, "Pool error: {}", msg),
            Error::TransactionError(msg) => write!(f, "Transaction error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

/// Convert tokio-postgres errors to Message DB errors
impl From<tokio_postgres::Error> for Error {
    fn from(err: tokio_postgres::Error) -> Self {
        // Check for specific error conditions
        if let Some(db_error) = err.as_db_error() {
            let message = db_error.message();

            // Check for expected version mismatch (Message DB raises this as an error)
            if message.contains("Wrong expected version") || message.contains("stream version") {
                // Try to parse out the stream name and versions from the error message
                // This is a simplification - actual parsing would be more robust
                return Error::ConcurrencyError {
                    stream_name: "unknown".to_string(),
                    expected_version: -1,
                    actual_version: None,
                };
            }

            // Return the actual database error message
            return Error::DatabaseError(format!("{}: {}", db_error.code().code(), message));
        }

        // For non-database errors, show the full error
        Error::DatabaseError(format!("{:?}", err))
    }
}

/// Convert deadpool errors to Message DB errors
impl From<deadpool_postgres::PoolError> for Error {
    fn from(err: deadpool_postgres::PoolError) -> Self {
        Error::PoolError(err.to_string())
    }
}

/// Convert deadpool build errors to Message DB errors
impl From<deadpool_postgres::BuildError> for Error {
    fn from(err: deadpool_postgres::BuildError) -> Self {
        Error::ConnectionError(err.to_string())
    }
}

/// Convert UUID parse errors to Message DB errors
impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Self {
        Error::ValidationError(format!("Invalid UUID: {}", err))
    }
}

/// Convert JSON errors to Message DB errors
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::ValidationError(format!("JSON error: {}", err))
    }
}
