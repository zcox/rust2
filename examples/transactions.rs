/// Example: Transaction Patterns in Message DB
///
/// This example demonstrates transaction support for atomic operations:
/// - Atomic multi-message writes
/// - Read-process-write pattern
/// - Money transfer pattern (dual write)
/// - Transaction rollback on error
/// - Optimistic concurrency in transactions
///
/// To run this example:
/// 1. Start Message DB: docker-compose up -d
/// 2. Run: cargo run --example transactions

use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::message_db::StreamReadOptions;
use rust2::message_db::types::WriteMessage;
use serde_json::json;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Message DB Transactions Example ===\n");

    // Connect to Message DB
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store"
    )?;
    let client = MessageDbClient::new(config).await?;

    // 1. Basic transaction with multiple writes
    println!("1. Atomic multi-message write");
    println!("------------------------------");

    let order_id = Uuid::new_v4().to_string();
    let order_stream = format!("order-{}", order_id);

    let mut txn = client.begin_transaction().await?;

    // Write multiple events atomically
    let events = vec![
        ("OrderPlaced", json!({ "order_id": order_id, "items": ["item1", "item2"] })),
        ("PaymentRequested", json!({ "amount": 99.99, "currency": "USD" })),
        ("InventoryReserved", json!({ "items": ["item1", "item2"] })),
    ];

    for (event_type, data) in events {
        let event = WriteMessage::new(
            Uuid::new_v4(),
            &order_stream,
            event_type
        ).with_data(data);

        let position = txn.write_message(event).await?;
        println!("  Wrote {} at position {}", event_type, position);
    }

    txn.commit().await?;
    println!("✓ Transaction committed - all 3 events written atomically\n");

    // 2. Money transfer pattern (dual write)
    println!("2. Money transfer (dual write)");
    println!("-------------------------------");

    let account1_id = Uuid::new_v4().to_string();
    let account2_id = Uuid::new_v4().to_string();
    let account1_stream = format!("account-{}", account1_id);
    let account2_stream = format!("account-{}", account2_id);

    // First, create both accounts with initial balance
    for (stream, balance) in [(account1_stream.as_str(), 1000), (account2_stream.as_str(), 500)] {
        let event = WriteMessage::new(
            Uuid::new_v4(),
            stream,
            "AccountOpened"
        ).with_data(json!({ "initial_balance": balance }));

        client.write_message(event).await?;
    }

    // Now transfer money atomically
    let transfer_amount = 250;
    let transfer_id = Uuid::new_v4().to_string();

    let mut txn = client.begin_transaction().await?;

    // Debit from account 1
    let debit_event = WriteMessage::new(
        Uuid::new_v4(),
        &account1_stream,
        "Withdrawn"
    )
    .with_data(json!({
        "amount": transfer_amount,
        "transfer_id": transfer_id
    }))
    .with_metadata(json!({
        "correlation_id": transfer_id
    }));

    let pos1 = txn.write_message(debit_event).await?;
    println!("  Debited ${} from account1 (position {})", transfer_amount, pos1);

    // Credit to account 2
    let credit_event = WriteMessage::new(
        Uuid::new_v4(),
        &account2_stream,
        "Deposited"
    )
    .with_data(json!({
        "amount": transfer_amount,
        "transfer_id": transfer_id
    }))
    .with_metadata(json!({
        "correlation_id": transfer_id
    }));

    let pos2 = txn.write_message(credit_event).await?;
    println!("  Credited ${} to account2 (position {})", transfer_amount, pos2);

    txn.commit().await?;
    println!("✓ Transfer completed atomically\n");

    // 3. Read-process-write pattern
    println!("3. Read-process-write pattern");
    println!("------------------------------");

    let account3_id = Uuid::new_v4().to_string();
    let account3_stream = format!("account-{}", account3_id);

    // Create account with some history
    for event_type in ["AccountOpened", "Deposited", "Withdrawn"].iter() {
        let data = match *event_type {
            "AccountOpened" => json!({ "initial_balance": 1000 }),
            "Deposited" => json!({ "amount": 500 }),
            "Withdrawn" => json!({ "amount": 200 }),
            _ => json!({}),
        };

        let event = WriteMessage::new(Uuid::new_v4(), &account3_stream, *event_type)
            .with_data(data);
        client.write_message(event).await?;
        println!("  Setup: wrote {}", event_type);
    }

    // Now do read-process-write in a transaction
    let mut txn = client.begin_transaction().await?;

    // Read current state
    let options = StreamReadOptions::new(&account3_stream);
    let messages = txn.get_stream_messages(options).await?;
    println!("\n  Read {} events from stream", messages.len());

    // Calculate current balance (simple projection)
    let mut balance = 0;
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
    println!("  Calculated balance: ${}", balance);

    // Get version for optimistic concurrency
    let current_version = txn.stream_version(&account3_stream).await?;
    println!("  Current stream version: {:?}", current_version);

    // Make business decision and write
    let withdrawal_amount = 300;
    if balance >= withdrawal_amount {
        let event = WriteMessage::new(
            Uuid::new_v4(),
            &account3_stream,
            "Withdrawn"
        )
        .with_data(json!({ "amount": withdrawal_amount }))
        .with_expected_version(current_version.unwrap());

        let position = txn.write_message(event).await?;
        println!("  Wrote withdrawal at position {}", position);

        txn.commit().await?;
        println!("✓ Read-process-write completed successfully\n");
    } else {
        txn.rollback().await?;
        println!("✗ Insufficient funds - transaction rolled back\n");
    }

    // 4. Transaction rollback on error
    println!("4. Transaction rollback on error");
    println!("---------------------------------");

    let test_stream = format!("test-{}", Uuid::new_v4());

    // Start a transaction
    let mut txn = client.begin_transaction().await?;

    // Write first message
    let event1 = WriteMessage::new(Uuid::new_v4(), &test_stream, "Event1")
        .with_data(json!({ "value": 1 }));
    let pos1 = txn.write_message(event1).await?;
    println!("  Wrote Event1 at position {}", pos1);

    // Write second message
    let event2 = WriteMessage::new(Uuid::new_v4(), &test_stream, "Event2")
        .with_data(json!({ "value": 2 }));
    let pos2 = txn.write_message(event2).await?;
    println!("  Wrote Event2 at position {}", pos2);

    // Simulate error and rollback
    println!("  Simulating error condition...");
    txn.rollback().await?;
    println!("✓ Transaction rolled back\n");

    // Verify stream is empty
    let options = StreamReadOptions::new(&test_stream);
    let messages = client.get_stream_messages(options).await?;
    println!("  Verified: stream has {} messages (both writes were rolled back)", messages.len());

    // 5. Committing transaction
    println!("\n5. Successfully committing transaction");
    println!("---------------------------------------");

    let mut txn = client.begin_transaction().await?;

    let event1 = WriteMessage::new(Uuid::new_v4(), &test_stream, "Event1")
        .with_data(json!({ "value": 1 }));
    txn.write_message(event1).await?;
    println!("  Wrote Event1");

    let event2 = WriteMessage::new(Uuid::new_v4(), &test_stream, "Event2")
        .with_data(json!({ "value": 2 }));
    txn.write_message(event2).await?;
    println!("  Wrote Event2");

    txn.commit().await?;
    println!("✓ Transaction committed\n");

    // Verify both messages were written
    let messages = client.get_stream_messages(StreamReadOptions::new(&test_stream)).await?;
    println!("  Verified: stream has {} messages", messages.len());

    println!("\n=== Summary ===");
    println!("Successfully demonstrated:");
    println!("  • Atomic multi-message writes");
    println!("  • Money transfer pattern (dual write)");
    println!("  • Read-process-write pattern");
    println!("  • Transaction rollback");
    println!("  • Transaction commit");
    println!("\nTransactions ensure all-or-nothing semantics for complex operations!");

    Ok(())
}
