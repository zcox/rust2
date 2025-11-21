use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::message_db::consumer::{Consumer, ConsumerConfig};
use rust2::message_db::types::{Message, WriteMessage};
use serde_json::json;
use uuid::Uuid;

/// Example: Message DB Consumer Pattern
///
/// This example demonstrates how to use the Consumer to process messages
/// from a category with automatic position tracking and message dispatching.
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example consumer_example

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Consumer Example ===\n");

    // Create client
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store"
    )?;
    let client = MessageDbClient::new(config).await?;

    // First, write some test messages to the account category
    println!("Writing test messages...");

    for i in 0..5 {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            format!("account-{}", i),
            "Withdrawn"
        )
        .with_data(json!({
            "amount": 10 * (i + 1),
            "currency": "USD"
        }))
        .with_metadata(json!({
            "correlation_id": format!("corr-{}", i)
        }));

        client.write_message(msg).await?;
    }

    for i in 0..3 {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            format!("account-{}", i),
            "Deposited"
        )
        .with_data(json!({
            "amount": 20 * (i + 1),
            "currency": "USD"
        }));

        client.write_message(msg).await?;
    }

    println!("Wrote 8 test messages\n");

    // Configure consumer
    let consumer_config = ConsumerConfig::new("account", "example-consumer")
        .with_batch_size(3)  // Small batch for demonstration
        .with_polling_interval_ms(1000)  // 1 second between polls
        .with_position_update_interval(2);  // Write position every 2 messages

    // Create consumer
    let mut consumer = Consumer::new(client.clone(), consumer_config).await?;

    println!("Consumer starting from position: {}", consumer.current_position());
    println!("Position stream: {}\n", consumer.position_stream_name());

    // Register message handlers
    consumer.on("Withdrawn", |msg: Message| {
        Box::pin(async move {
            println!(
                "[Withdrawn] Stream: {}, Amount: ${}, Position: {}",
                msg.stream_name,
                msg.data["amount"],
                msg.global_position
            );
            Ok(())
        })
    });

    consumer.on("Deposited", |msg: Message| {
        Box::pin(async move {
            println!(
                "[Deposited] Stream: {}, Amount: ${}, Position: {}",
                msg.stream_name,
                msg.data["amount"],
                msg.global_position
            );
            Ok(())
        })
    });

    // Process messages (poll a few times for demonstration)
    println!("Processing messages...\n");

    for round in 1..=3 {
        println!("--- Polling Round {} ---", round);
        let had_messages = consumer.poll_once().await?;

        if had_messages {
            println!("Current position: {}", consumer.current_position());
        } else {
            println!("No messages available");
        }
        println!();
    }

    // Flush position before exiting
    consumer.flush_position().await?;
    println!("Final position saved: {}", consumer.current_position());

    println!("\n=== Example Complete ===");
    println!("The consumer has processed all messages and saved its position.");
    println!("If you run this example again, it will resume from where it left off.");

    Ok(())
}
