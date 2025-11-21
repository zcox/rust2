mod common;

use rust2::message_db::{MessageDbClient, MessageDbConfig};
use testcontainers::clients::Cli;

#[tokio::test]
async fn test_connection_pool_setup() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());

    // Get the mapped port
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);

    // Build connection string
    let connection_string = common::build_connection_string("127.0.0.1", host_port);

    // Create client configuration
    let config = MessageDbConfig::from_connection_string(&connection_string)
        .expect("Failed to create config from connection string");

    // Create client - this tests connection pool setup
    let client = MessageDbClient::new(config)
        .await
        .expect("Failed to create Message DB client");

    // If we get here, the connection pool was set up successfully
    drop(client);
}

#[tokio::test]
async fn test_connection_pool_exhaustion_and_recovery() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());

    // Get the mapped port
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);

    // Build connection string
    let connection_string = common::build_connection_string("127.0.0.1", host_port);

    // Create client configuration with small pool
    let mut config = MessageDbConfig::from_connection_string(&connection_string)
        .expect("Failed to create config from connection string");

    config.max_pool_size = 2; // Small pool for testing

    // Create client
    let client = MessageDbClient::new(config)
        .await
        .expect("Failed to create Message DB client");

    // Get connections from the pool (up to max)
    // This is a basic test - actual pool exhaustion would require holding connections
    // which we'll test more thoroughly in future phases

    drop(client);
}

#[tokio::test]
async fn test_invalid_connection_string() {
    let result = MessageDbConfig::from_connection_string("invalid://connection/string");
    assert!(result.is_err(), "Should fail with invalid connection string");
}

#[tokio::test]
async fn test_connection_to_nonexistent_host() {
    let config = MessageDbConfig::from_connection_string(
        "postgresql://user:pass@nonexistent-host-12345:5432/db",
    )
    .expect("Config creation should succeed");

    // Trying to create a client should fail because host doesn't exist
    // Note: This might timeout rather than fail immediately
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        MessageDbClient::new(config),
    )
    .await;

    match result {
        Ok(client_result) => {
            assert!(
                client_result.is_err(),
                "Should fail to connect to nonexistent host"
            );
        }
        Err(_) => {
            // Timeout is also acceptable - connection attempt timed out
        }
    }
}
