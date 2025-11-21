/// Example: Optimistic Concurrency Control
///
/// This example demonstrates Message DB's optimistic concurrency control using
/// expected_version to prevent lost updates and ensure consistency.
///
/// Optimistic concurrency prevents:
/// - Lost updates (two processes overwriting each other)
/// - Race conditions in business logic
/// - Inconsistent state from concurrent operations
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example optimistic_concurrency

use rust2::message_db::{MessageDbClient, MessageDbConfig, Error};
use rust2::message_db::StreamReadOptions;
use rust2::message_db::types::WriteMessage;
use serde_json::json;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Optimistic Concurrency Example ===\n");

    // Connect to Message DB
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store"
    )?;
    let client = MessageDbClient::new(config).await?;

    // 1. Setup: Create an account
    println!("1. Setting up test account");
    println!("--------------------------");

    let account_id = Uuid::new_v4().to_string();
    let stream_name = format!("account-{}", account_id);

    let event = WriteMessage::new(
        Uuid::new_v4(),
        &stream_name,
        "AccountOpened"
    ).with_data(json!({ "initial_balance": 1000 }));

    client.write_message(event).await?;
    println!("✓ Created account with stream: {}", stream_name);

    // Get current version
    let version = client.stream_version(&stream_name).await?;
    println!("  Current version: {:?}\n", version);

    // 2. Successful write with expected_version
    println!("2. Successful write with expected_version");
    println!("------------------------------------------");

    let current_version = version.expect("Stream should exist");
    println!("Current version: {}", current_version);

    let event = WriteMessage::new(
        Uuid::new_v4(),
        &stream_name,
        "Deposited"
    )
    .with_data(json!({ "amount": 500 }))
    .with_expected_version(current_version);

    match client.write_message(event).await {
        Ok(position) => {
            println!("✓ Write succeeded at position {}", position);
            println!("  Expected version {} matched actual version\n", current_version);
        }
        Err(e) => {
            println!("✗ Write failed: {}", e);
        }
    }

    // 3. Failed write with wrong expected_version
    println!("3. Failed write with wrong expected_version");
    println!("--------------------------------------------");

    let wrong_version = 99; // Intentionally wrong
    println!("Attempting write with expected_version: {}", wrong_version);

    let event = WriteMessage::new(
        Uuid::new_v4(),
        &stream_name,
        "Withdrawn"
    )
    .with_data(json!({ "amount": 100 }))
    .with_expected_version(wrong_version);

    match client.write_message(event).await {
        Ok(position) => {
            println!("✗ Unexpected success at position {}", position);
        }
        Err(Error::ConcurrencyError { stream_name: err_stream, expected_version, actual_version }) => {
            println!("✓ Correctly rejected due to version mismatch");
            println!("  Stream: {}", err_stream);
            println!("  Expected: {}", expected_version);
            println!("  Actual: {:?}\n", actual_version);
        }
        Err(e) => {
            println!("✗ Unexpected error: {}\n", e);
        }
    }

    // 4. Retry pattern with version check
    println!("4. Retry pattern with version check");
    println!("------------------------------------");

    println!("Simulating concurrent modification scenario...\n");

    // This function demonstrates the retry pattern
    async fn withdraw_with_retry(
        client: &MessageDbClient,
        stream_name: &str,
        amount: i64,
        max_retries: u32,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        for attempt in 1..=max_retries {
            println!("  Attempt {}/{}", attempt, max_retries);

            // Read current state
            let options = StreamReadOptions::new(stream_name);
            let messages = client.get_stream_messages(options).await?;

            // Calculate current balance
            let mut balance = 0i64;
            for msg in &messages {
                match msg.message_type.as_str() {
                    "AccountOpened" => {
                        balance = msg.data["initial_balance"].as_i64().unwrap_or(0);
                    }
                    "Deposited" => {
                        balance += msg.data["amount"].as_i64().unwrap_or(0);
                    }
                    "Withdrawn" => {
                        balance -= msg.data["amount"].as_i64().unwrap_or(0);
                    }
                    _ => {}
                }
            }

            println!("    Current balance: ${}", balance);

            // Check business rule
            if balance < amount {
                return Err("Insufficient funds".into());
            }

            // Get current version
            let current_version = client.stream_version(stream_name).await?
                .expect("Stream should exist");

            println!("    Current version: {}", current_version);

            // Try to write with expected version
            let event = WriteMessage::new(
                Uuid::new_v4(),
                stream_name,
                "Withdrawn"
            )
            .with_data(json!({ "amount": amount }))
            .with_expected_version(current_version);

            match client.write_message(event).await {
                Ok(position) => {
                    println!("    ✓ Write succeeded at position {}", position);
                    return Ok(position);
                }
                Err(Error::ConcurrencyError { .. }) => {
                    println!("    ✗ Concurrency conflict detected, retrying...");
                    // Continue to next attempt
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        Err("Max retries exceeded".into())
    }

    // Attempt withdrawal with retry
    match withdraw_with_retry(&client, &stream_name, 300, 5).await {
        Ok(position) => {
            println!("\n✓ Withdrawal completed at position {}", position);
        }
        Err(e) => {
            println!("\n✗ Withdrawal failed: {}", e);
        }
    }

    // 5. Demonstrate race condition prevention
    println!("\n5. Race condition prevention");
    println!("----------------------------");

    println!("\nOptimistic concurrency prevents these scenarios:");
    println!("  Scenario 1: Lost Update");
    println!("    - Process A reads stream (version 5)");
    println!("    - Process B reads stream (version 5)");
    println!("    - Process A writes event (version -> 6)");
    println!("    - Process B tries to write (expects version 5)");
    println!("    → Process B's write is REJECTED (actual version is 6)");
    println!("    → Process B must re-read, recalculate, and retry");

    println!("\n  Scenario 2: Double Spending");
    println!("    - Two withdrawal requests arrive simultaneously");
    println!("    - Both read balance = $100");
    println!("    - Both try to withdraw $80");
    println!("    - First write succeeds (balance -> $20)");
    println!("    - Second write is REJECTED (version mismatch)");
    println!("    - Second request re-reads balance ($20)");
    println!("    - Second withdrawal properly rejected (insufficient funds)");

    // 6. Final verification
    println!("\n6. Final verification");
    println!("--------------------");

    let options = StreamReadOptions::new(&stream_name);
    let messages = client.get_stream_messages(options).await?;

    println!("Stream '{}' contains {} events:", stream_name, messages.len());
    for msg in &messages {
        println!("  Position {}: {}", msg.position, msg.message_type);
        if let Some(amount) = msg.data.get("amount") {
            println!("    Amount: ${}", amount);
        }
    }

    let final_version = client.stream_version(&stream_name).await?;
    println!("\nFinal stream version: {:?}", final_version);

    println!("\n=== Summary ===");
    println!("Optimistic concurrency control provides:");
    println!("  • Protection against lost updates");
    println!("  • Consistent business logic execution");
    println!("  • Automatic conflict detection");
    println!("  • Simple retry patterns");
    println!("\nBest practices:");
    println!("  1. Always read current state first");
    println!("  2. Calculate new state based on current state");
    println!("  3. Write with expected_version set");
    println!("  4. Handle ConcurrencyError by retrying");
    println!("  5. Limit retry attempts to prevent infinite loops");

    Ok(())
}
