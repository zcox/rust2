use testcontainers::{core::WaitFor, GenericImage, RunnableImage};

/// The Message DB Docker image to use for testing
pub const MESSAGE_DB_IMAGE: &str = "ethangarofolo/message-db";
pub const MESSAGE_DB_TAG: &str = "1.3.1";

/// Default PostgreSQL port
pub const POSTGRES_PORT: u16 = 5432;

/// Default credentials for the Message DB container
pub const POSTGRES_USER: &str = "postgres";
pub const POSTGRES_PASSWORD: &str = "message_store_password";
pub const POSTGRES_DB: &str = "message_store";

/// Create a runnable Message DB container
pub fn create_message_db_container() -> RunnableImage<GenericImage> {
    let image = GenericImage::new(MESSAGE_DB_IMAGE, MESSAGE_DB_TAG)
        .with_env_var("POSTGRES_PASSWORD", POSTGRES_PASSWORD)
        .with_wait_for(WaitFor::message_on_stderr("database system is ready to accept connections"));

    RunnableImage::from(image).with_tag(MESSAGE_DB_TAG)
}

/// Build a connection string for the running Message DB container
pub fn build_connection_string(host: &str, port: u16) -> String {
    format!(
        "postgresql://{}:{}@{}:{}/{}",
        POSTGRES_USER, POSTGRES_PASSWORD, host, port, POSTGRES_DB
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_connection_string() {
        let conn_str = build_connection_string("localhost", 5433);
        assert_eq!(
            conn_str,
            "postgresql://postgres:message_store_password@localhost:5433/message_store"
        );
    }
}
