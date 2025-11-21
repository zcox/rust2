/// Example: Writing Events to Message DB
///
/// This example demonstrates various patterns for writing events to Message DB:
/// - Basic event writing
/// - Events with data and metadata
/// - Writing multiple events to a stream
/// - Idempotent writes (duplicate message IDs)
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example writing_events

use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::message_db::types::WriteMessage;
use serde_json::json;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Writing Events Example ===\n");

    // Connect to Message DB
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store"
    )?;
    let client = MessageDbClient::new(config).await?;

    // 1. Basic event write
    println!("1. Writing a basic event");
    println!("------------------------");

    let account_id = Uuid::new_v4().to_string();
    let stream_name = format!("account-{}", account_id);

    let event_id = Uuid::new_v4();
    let event = WriteMessage::new(
        event_id,
        &stream_name,
        "AccountOpened"
    )
    .with_data(json!({
        "account_id": account_id,
        "initial_balance": 1000,
        "currency": "USD"
    }));

    let position = client.write_message(event).await?;
    println!("✓ Wrote AccountOpened event to stream '{}' at position {}", stream_name, position);

    // 2. Event with metadata
    println!("\n2. Writing event with metadata");
    println!("-------------------------------");

    let correlation_id = Uuid::new_v4().to_string();
    let causation_id = event_id.to_string();

    let event = WriteMessage::new(
        Uuid::new_v4(),
        &stream_name,
        "Deposited"
    )
    .with_data(json!({
        "amount": 500,
        "currency": "USD",
        "description": "Initial deposit"
    }))
    .with_metadata(json!({
        "correlation_id": correlation_id,
        "causation_id": causation_id,
        "user_id": "user-123",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }));

    let position = client.write_message(event).await?;
    println!("✓ Wrote Deposited event with metadata at position {}", position);
    println!("  Correlation ID: {}", correlation_id);

    // 3. Writing multiple events to build up stream state
    println!("\n3. Writing multiple events");
    println!("--------------------------");

    let events = vec![
        ("Withdrawn", json!({ "amount": 100, "description": "ATM withdrawal" })),
        ("Deposited", json!({ "amount": 250, "description": "Paycheck" })),
        ("Withdrawn", json!({ "amount": 75, "description": "Online purchase" })),
        ("Deposited", json!({ "amount": 100, "description": "Refund" })),
    ];

    for (event_type, data) in events {
        let event = WriteMessage::new(
            Uuid::new_v4(),
            &stream_name,
            event_type
        ).with_data(data);

        let position = client.write_message(event).await?;
        println!("✓ Wrote {} event at position {}", event_type, position);
    }

    // 4. Idempotent writes (duplicate message ID)
    println!("\n4. Demonstrating idempotent writes");
    println!("-----------------------------------");

    let duplicate_id = Uuid::new_v4();
    let event = WriteMessage::new(
        duplicate_id,
        &stream_name,
        "Withdrawn"
    ).with_data(json!({ "amount": 50 }));

    let position1 = client.write_message(event.clone()).await?;
    println!("✓ First write at position {}", position1);

    // Write again with same message ID
    let position2 = client.write_message(event).await?;
    println!("✓ Second write returned position {} (idempotent - no duplicate created)", position2);

    if position1 == position2 {
        println!("  ✓ Confirmed: both writes returned same position");
    }

    // 5. Writing to command stream (category with type)
    println!("\n5. Writing to command stream");
    println!("-----------------------------");

    let command_stream = format!("account:command-{}", Uuid::new_v4());
    let command = WriteMessage::new(
        Uuid::new_v4(),
        &command_stream,
        "WithdrawMoney"
    )
    .with_data(json!({
        "account_id": account_id,
        "amount": 200,
        "reason": "Customer request"
    }))
    .with_metadata(json!({
        "correlation_id": Uuid::new_v4().to_string(),
        "reply_stream_name": format!("account:reply-{}", Uuid::new_v4())
    }));

    let position = client.write_message(command).await?;
    println!("✓ Wrote command to '{}' at position {}", command_stream, position);

    // Summary
    println!("\n=== Summary ===");
    println!("Successfully demonstrated:");
    println!("  • Basic event writing");
    println!("  • Events with data and metadata");
    println!("  • Multiple events to same stream");
    println!("  • Idempotent writes");
    println!("  • Command stream writing");
    println!("\nStream '{}' now contains multiple events", stream_name);
    println!("Run the reading_streams example to view these events!");

    Ok(())
}
