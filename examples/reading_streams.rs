/// Example: Reading Streams from Message DB
///
/// This example demonstrates various patterns for reading messages:
/// - Reading all messages from a stream
/// - Reading with position offset
/// - Reading with batch size limits
/// - Reading from categories
/// - Getting the last message
/// - Getting stream version
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example reading_streams

use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::message_db::{StreamReadOptions, CategoryReadOptions};
use rust2::message_db::types::WriteMessage;
use serde_json::json;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Reading Streams Example ===\n");

    // Connect to Message DB
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store"
    )?;
    let client = MessageDbClient::new(config).await?;

    // First, write some test data
    println!("Setting up test data...");
    let account_id = Uuid::new_v4().to_string();
    let stream_name = format!("account-{}", account_id);

    for i in 0..10 {
        let event_type = if i % 2 == 0 { "Deposited" } else { "Withdrawn" };
        let amount = (i + 1) * 100;

        let event = WriteMessage::new(
            Uuid::new_v4(),
            &stream_name,
            event_type
        )
        .with_data(json!({
            "amount": amount,
            "currency": "USD",
            "sequence": i
        }))
        .with_metadata(json!({
            "correlation_id": format!("corr-{}", i)
        }));

        client.write_message(event).await?;
    }
    println!("✓ Created test stream '{}' with 10 events\n", stream_name);

    // 1. Read all messages from a stream
    println!("1. Reading all messages from stream");
    println!("------------------------------------");

    let options = StreamReadOptions::new(&stream_name);
    let messages = client.get_stream_messages(options).await?;

    println!("Found {} messages:", messages.len());
    for msg in &messages {
        println!("  Position {}: {} - amount: ${}",
            msg.position,
            msg.message_type,
            msg.data["amount"]
        );
    }

    // 2. Read with position offset (skip first 5 messages)
    println!("\n2. Reading from position 5 onwards");
    println!("-----------------------------------");

    let options = StreamReadOptions::new(&stream_name).with_position(5);
    let messages = client.get_stream_messages(options).await?;

    println!("Found {} messages starting from position 5:", messages.len());
    for msg in &messages {
        println!("  Position {}: {}", msg.position, msg.message_type);
    }

    // 3. Read with batch size limit
    println!("\n3. Reading with batch size limit");
    println!("---------------------------------");

    let options = StreamReadOptions::new(&stream_name).with_batch_size(3);
    let messages = client.get_stream_messages(options).await?;

    println!("Requested batch size 3, got {} messages:", messages.len());
    for msg in &messages {
        println!("  Position {}: {}", msg.position, msg.message_type);
    }

    // 4. Get last message from stream
    println!("\n4. Getting last message from stream");
    println!("------------------------------------");

    match client.get_last_stream_message(&stream_name, None).await? {
        Some(msg) => {
            println!("Last message in stream:");
            println!("  Type: {}", msg.message_type);
            println!("  Position: {}", msg.position);
            println!("  Amount: ${}", msg.data["amount"]);
            println!("  Time: {}", msg.time);
        }
        None => println!("Stream is empty"),
    }

    // 5. Get last message of specific type
    println!("\n5. Getting last message of specific type");
    println!("-----------------------------------------");

    match client.get_last_stream_message(&stream_name, Some("Withdrawn")).await? {
        Some(msg) => {
            println!("Last 'Withdrawn' message:");
            println!("  Position: {}", msg.position);
            println!("  Amount: ${}", msg.data["amount"]);
        }
        None => println!("No Withdrawn messages found"),
    }

    // 6. Get stream version
    println!("\n6. Getting stream version");
    println!("-------------------------");

    match client.stream_version(&stream_name).await? {
        Some(version) => {
            println!("Stream version: {}", version);
            println!("(Version is the position of the last message)");
        }
        None => println!("Stream does not exist"),
    }

    // 7. Read from category (all account streams)
    println!("\n7. Reading from category");
    println!("------------------------");

    // Write to a few more account streams
    for i in 0..3 {
        let other_stream = format!("account-{}", Uuid::new_v4());
        let event = WriteMessage::new(
            Uuid::new_v4(),
            &other_stream,
            "AccountOpened"
        ).with_data(json!({ "account_number": i }));

        client.write_message(event).await?;
    }

    let options = CategoryReadOptions::new("account")
        .with_batch_size(20);
    let messages = client.get_category_messages(options).await?;

    println!("Found {} messages in 'account' category:", messages.len());

    // Group by stream
    let mut streams = std::collections::HashMap::new();
    for msg in &messages {
        *streams.entry(&msg.stream_name).or_insert(0) += 1;
    }

    println!("Messages grouped by stream:");
    for (stream, count) in streams {
        println!("  {}: {} messages", stream, count);
    }

    // 8. Read category from specific position
    println!("\n8. Reading category from specific global position");
    println!("--------------------------------------------------");

    // Get a global position from middle of our data
    if let Some(msg) = messages.get(5) {
        let start_position = msg.global_position;

        let options = CategoryReadOptions::new("account")
            .with_position(start_position)
            .with_batch_size(10);
        let messages = client.get_category_messages(options).await?;

        println!("Reading from global position {}, got {} messages",
            start_position, messages.len());
    }

    // 9. Demonstrate message properties
    println!("\n9. Message properties");
    println!("---------------------");

    if let Some(msg) = client.get_last_stream_message(&stream_name, None).await? {
        println!("Message structure:");
        println!("  ID: {}", msg.id);
        println!("  Stream: {}", msg.stream_name);
        println!("  Type: {}", msg.message_type);
        println!("  Position (stream): {}", msg.position);
        println!("  Global Position: {}", msg.global_position);
        println!("  Time: {}", msg.time);
        println!("  Data: {}", msg.data);
        println!("  Metadata: {:?}", msg.metadata);

        if let Some(corr_id) = msg.correlation_id() {
            println!("  Correlation ID: {}", corr_id);
        }
    }

    println!("\n=== Summary ===");
    println!("Successfully demonstrated:");
    println!("  • Reading all messages from a stream");
    println!("  • Reading with position offset");
    println!("  • Reading with batch size limits");
    println!("  • Getting last message (all types and filtered)");
    println!("  • Getting stream version");
    println!("  • Reading from categories");
    println!("  • Message properties and metadata");

    Ok(())
}
