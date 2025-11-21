/// Example: Consumer Groups for Horizontal Scaling
///
/// This example demonstrates consumer groups, which distribute messages across
/// multiple consumer instances based on stream name hash for horizontal scaling.
///
/// Consumer groups enable:
/// - Load balancing across multiple workers
/// - Parallel processing of different entity streams
/// - Fault tolerance (workers can be added/removed)
/// - Consistent routing (same stream always goes to same member)
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example consumer_groups

use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::message_db::consumer::{Consumer, ConsumerConfig};
use rust2::message_db::types::WriteMessage;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Consumer Groups Example ===\n");

    // Connect to Message DB
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store"
    )?;
    let client = MessageDbClient::new(config).await?;

    // 1. Setup: Write test messages to different account streams
    println!("1. Setting up test data");
    println!("-----------------------");

    let num_accounts = 12;
    let mut account_ids = Vec::new();

    println!("Writing messages to {} different account streams...", num_accounts);

    for i in 0..num_accounts {
        let account_id = format!("acc-{:03}", i);
        account_ids.push(account_id.clone());
        let stream_name = format!("account-{}", account_id);

        // Write a few events to each account
        for j in 0..3 {
            let event_type = if j % 2 == 0 { "Deposited" } else { "Withdrawn" };
            let event = WriteMessage::new(
                Uuid::new_v4(),
                &stream_name,
                event_type
            )
            .with_data(json!({
                "amount": (j + 1) * 100,
                "account_id": account_id
            }));

            client.write_message(event).await?;
        }
    }

    println!("✓ Wrote {} events across {} streams\n", num_accounts * 3, num_accounts);

    // 2. Create consumer group with 3 members
    println!("2. Creating consumer group with 3 members");
    println!("------------------------------------------");

    let group_size = 3;
    let mut consumers = Vec::new();

    // Shared state to track which messages each consumer processes
    let consumer_stats: Arc<Mutex<HashMap<usize, Vec<String>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    for member_id in 0..group_size {
        println!("  Creating consumer group member {}/{}", member_id, group_size - 1);

        // Configure consumer with consumer group settings
        let consumer_config = ConsumerConfig::new("account", &format!("worker-{}", member_id))
            .with_consumer_group(member_id as i64, group_size)
            .with_batch_size(20)
            .with_position_update_interval(5);

        let consumer = Consumer::new(client.clone(), consumer_config).await?;

        println!("    Position stream: {}", consumer.position_stream_name());
        println!("    Starting position: {}", consumer.current_position());

        consumers.push(consumer);
    }

    println!("\n3. Processing messages with consumer group");
    println!("-------------------------------------------");

    // Process messages with each consumer
    for (member_id, consumer) in consumers.iter_mut().enumerate() {
        let stats_deposited = consumer_stats.clone();
        let stats_withdrawn = consumer_stats.clone();
        let member_id_copy = member_id;

        // Register handler that tracks which streams this member processes
        consumer.on("Deposited", move |msg| {
            let stats = stats_deposited.clone();
            let stream_name = msg.stream_name.clone();

            Box::pin(async move {
                let mut stats = stats.lock().unwrap();
                stats.entry(member_id_copy)
                    .or_insert_with(Vec::new)
                    .push(stream_name);
                Ok(())
            })
        });

        consumer.on("Withdrawn", move |msg| {
            let stats = stats_withdrawn.clone();
            let stream_name = msg.stream_name.clone();

            Box::pin(async move {
                let mut stats = stats.lock().unwrap();
                stats.entry(member_id_copy)
                    .or_insert_with(Vec::new)
                    .push(stream_name);
                Ok(())
            })
        });

        // Poll once to process available messages
        let had_messages = consumer.poll_once().await?;

        if had_messages {
            println!("  Worker {} processed messages", member_id);
        } else {
            println!("  Worker {} had no messages", member_id);
        }
    }

    // Flush positions for all consumers
    for consumer in consumers.iter_mut() {
        consumer.flush_position().await?;
    }

    // 4. Analyze distribution
    println!("\n4. Analyzing message distribution");
    println!("----------------------------------");

    let stats = consumer_stats.lock().unwrap();

    println!("\nMessages processed by each worker:");
    for member_id in 0..group_size {
        let member_id_key = member_id as usize;
        if let Some(streams) = stats.get(&member_id_key) {
            // Count unique streams
            let mut unique_streams: Vec<_> = streams.clone();
            unique_streams.sort();
            unique_streams.dedup();

            println!("  Worker {}: {} messages from {} unique streams",
                member_id,
                streams.len(),
                unique_streams.len()
            );

            // Show first few unique streams
            let preview: Vec<_> = unique_streams.iter().take(3).collect();
            println!("    Streams (first 3): {:?}", preview);
        } else {
            println!("  Worker {}: 0 messages", member_id);
        }
    }

    // Verify all messages were processed
    let total_processed: usize = stats.values().map(|v| v.len()).sum();
    println!("\nTotal messages processed: {} (expected: {})",
        total_processed, num_accounts * 3);

    // 5. Demonstrate consistent routing
    println!("\n5. Demonstrating consistent routing");
    println!("------------------------------------");

    println!("\nConsumer groups ensure:");
    println!("  ✓ Each message is processed by exactly one consumer");
    println!("  ✓ Messages from the same stream always go to the same consumer");
    println!("  ✓ Load is balanced across consumers based on stream hash");
    println!("  ✓ Consumers can be scaled horizontally by changing group size");

    // Show which worker would get specific streams (deterministic)
    println!("\nStream routing is deterministic:");
    for i in 0..3 {
        let account_id = format!("acc-{:03}", i);
        let stream_name = format!("account-{}", account_id);

        // Determine which worker processes this stream
        let worker_id = stats.iter()
            .find(|(_worker_id, streams)| {
                streams.iter().any(|s| s == &stream_name)
            })
            .map(|(id, _)| id);

        if let Some(worker_id) = worker_id {
            println!("  '{}' -> Worker {}", stream_name, worker_id);
        }
    }

    println!("\n=== Summary ===");
    println!("Consumer groups enable horizontal scaling:");
    println!("  • Multiple workers process messages in parallel");
    println!("  • Messages are distributed based on stream name hash");
    println!("  • Same stream always routed to same worker");
    println!("  • Each position tracked independently per worker");
    println!("  • Workers can be added/removed to scale");
    println!("\nUse consumer groups to:");
    println!("  - Scale message processing horizontally");
    println!("  - Ensure ordered processing per entity stream");
    println!("  - Achieve fault tolerance and high availability");

    Ok(())
}
