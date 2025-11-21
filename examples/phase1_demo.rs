/// Phase 1 Demo - Message DB Client Foundation
///
/// This example demonstrates the Phase 1 foundation functionality:
/// - Connection pool setup
/// - Stream name parsing utilities
/// - Error handling
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example phase1_demo

use rust2::message_db::{category, cardinal_id, id, is_category, MessageDbClient, MessageDbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Client - Phase 1 Demo ===\n");

    // 1. Demonstrate stream name parsing utilities
    println!("1. Stream Name Parsing Utilities");
    println!("---------------------------------");

    let stream_names = vec![
        "account-123",
        "account:command-456",
        "transaction:event+audit-xyz",
        "withdrawal:position-consumer-1",
        "account",
        "account:command",
    ];

    for stream_name in stream_names {
        println!("\nStream: '{}'", stream_name);
        println!("  Category: '{}'", category(stream_name));
        println!("  ID: {:?}", id(stream_name));
        println!("  Cardinal ID: {:?}", cardinal_id(stream_name));
        println!("  Is Category: {}", is_category(stream_name));
    }

    // 2. Demonstrate connection pool setup
    println!("\n\n2. Connection Pool Setup");
    println!("------------------------");

    // For local development with docker-compose
    let connection_string = "postgresql://postgres:message_store_password@localhost:5433/message_store";

    println!("Connection string: {}", connection_string);

    match MessageDbConfig::from_connection_string(connection_string) {
        Ok(config) => {
            println!("✓ Configuration created successfully");
            println!("  Host: {}", config.host);
            println!("  Port: {}", config.port);
            println!("  Database: {}", config.database);
            println!("  Max pool size: {}", config.max_pool_size);

            // Try to create client (will fail if Message DB is not running)
            match MessageDbClient::new(config).await {
                Ok(_client) => {
                    println!("✓ Successfully connected to Message DB!");
                    println!("  Connection pool is ready");
                }
                Err(e) => {
                    println!("✗ Failed to connect to Message DB");
                    println!("  Error: {}", e);
                    println!("\n  Tip: Start Message DB with docker-compose:");
                    println!("    docker-compose up -d");
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to create configuration: {}", e);
        }
    }

    // 3. Demonstrate error handling
    println!("\n\n3. Error Handling");
    println!("-----------------");

    println!("Testing invalid connection string...");
    match MessageDbConfig::from_connection_string("invalid://connection") {
        Ok(_) => println!("✗ Should have failed!"),
        Err(e) => println!("✓ Correctly rejected: {}", e),
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}
