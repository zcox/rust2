mod common;

use rust2::message_db::{
    CategoryReadOptions, MessageDbClient, MessageDbConfig, StreamReadOptions,
    WriteMessage,
};
use serde_json::json;
use testcontainers::clients::Cli;
use uuid::Uuid;

// Macro to set up test environment
// Note: This keeps _docker and _container alive for the duration of the test
macro_rules! setup_test {
    ($docker:ident, $container:ident, $client:ident) => {
        let $docker = Cli::default();
        let $container = $docker.run(common::create_message_db_container());

        // Give the container a moment to fully initialize
        // Message DB needs time to create its functions after PostgreSQL is ready
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let host_port = $container.get_host_port_ipv4(common::POSTGRES_PORT);
        let connection_string = common::build_connection_string("127.0.0.1", host_port);
        let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
        let $client = MessageDbClient::new(config).await.unwrap();
    };
}

// ============================================================================
// write_message tests
// ============================================================================

#[tokio::test]
async fn test_write_message_basic() {
    setup_test!(_docker, _container, client);

    let msg_id = Uuid::new_v4();
    let msg = WriteMessage::new(msg_id, "test-account-123", "Deposited")
        .with_data(json!({ "amount": 100, "currency": "USD" }))
        .with_metadata(json!({ "correlation_id": "test-corr-1" }));

    let position = client
        .write_message(msg)
        .await
        .expect("Failed to write message");

    // First message should be at position 0
    assert_eq!(position, 0);
}

#[tokio::test]
async fn test_write_message_multiple() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-account-456";

    // Write first message
    let msg1 = WriteMessage::new(Uuid::new_v4(), stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    let pos1 = client.write_message(msg1).await.unwrap();
    assert_eq!(pos1, 0);

    // Write second message
    let msg2 = WriteMessage::new(Uuid::new_v4(), stream_name, "Withdrawn")
        .with_data(json!({ "amount": 50 }));
    let pos2 = client.write_message(msg2).await.unwrap();
    assert_eq!(pos2, 1);

    // Write third message
    let msg3 = WriteMessage::new(Uuid::new_v4(), stream_name, "Withdrawn")
        .with_data(json!({ "amount": 25 }));
    let pos3 = client.write_message(msg3).await.unwrap();
    assert_eq!(pos3, 2);
}

#[tokio::test]
async fn test_write_message_idempotent() {
    setup_test!(_docker, _container, client);

    let msg_id = Uuid::new_v4();
    let stream_name = "test-account-789";

    // Write message first time
    let msg1 = WriteMessage::new(msg_id, stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    let pos1 = client.write_message(msg1).await.unwrap();

    // Write same message ID again - should be idempotent
    let msg2 = WriteMessage::new(msg_id, stream_name, "Deposited")
        .with_data(json!({ "amount": 200 })); // Different data, same ID
    let pos2 = client.write_message(msg2).await.unwrap();

    // Should return same position
    assert_eq!(pos1, pos2);
}

#[tokio::test]
async fn test_write_message_expected_version_success() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-account-version-1";

    // Write first message (no expected version)
    let msg1 = WriteMessage::new(Uuid::new_v4(), stream_name, "Opened")
        .with_data(json!({ "initial_balance": 0 }));
    client.write_message(msg1).await.unwrap();

    // Write second message with expected version 0 (should succeed)
    let msg2 = WriteMessage::new(Uuid::new_v4(), stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }))
        .with_expected_version(0);
    let pos = client.write_message(msg2).await.unwrap();
    assert_eq!(pos, 1);
}

#[tokio::test]
async fn test_write_message_expected_version_failure() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-account-version-2";

    // Write first message
    let msg1 = WriteMessage::new(Uuid::new_v4(), stream_name, "Opened")
        .with_data(json!({ "initial_balance": 0 }));
    client.write_message(msg1).await.unwrap();

    // Try to write with wrong expected version
    let msg2 = WriteMessage::new(Uuid::new_v4(), stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }))
        .with_expected_version(5); // Wrong version

    let result = client.write_message(msg2).await;
    assert!(result.is_err(), "Should fail with wrong expected version");
}

#[tokio::test]
async fn test_write_message_with_json_data() {
    setup_test!(_docker, _container, client);

    let complex_data = json!({
        "transaction_id": "txn-123",
        "items": [
            { "sku": "ITEM-1", "quantity": 2, "price": 10.50 },
            { "sku": "ITEM-2", "quantity": 1, "price": 25.00 }
        ],
        "total": 46.00,
        "customer": {
            "id": "cust-456",
            "name": "John Doe"
        }
    });

    let msg = WriteMessage::new(Uuid::new_v4(), "test-order-123", "OrderPlaced")
        .with_data(complex_data);

    let position = client.write_message(msg).await.unwrap();
    assert_eq!(position, 0);
}

// ============================================================================
// get_stream_messages tests
// ============================================================================

#[tokio::test]
async fn test_get_stream_messages_empty() {
    setup_test!(_docker, _container, client);

    let options = StreamReadOptions::new("nonexistent-stream");
    let messages = client.get_stream_messages(options).await.unwrap();

    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_get_stream_messages_basic() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-stream-read-1";

    // Write some messages
    for i in 0..5 {
        let msg = WriteMessage::new(Uuid::new_v4(), stream_name, "TestEvent")
            .with_data(json!({ "sequence": i }));
        client.write_message(msg).await.unwrap();
    }

    // Read all messages
    let options = StreamReadOptions::new(stream_name);
    let messages = client.get_stream_messages(options).await.unwrap();

    assert_eq!(messages.len(), 5);
    assert_eq!(messages[0].position, 0);
    assert_eq!(messages[4].position, 4);
}

#[tokio::test]
async fn test_get_stream_messages_with_position() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-stream-read-2";

    // Write 10 messages
    for i in 0..10 {
        let msg = WriteMessage::new(Uuid::new_v4(), stream_name, "TestEvent")
            .with_data(json!({ "sequence": i }));
        client.write_message(msg).await.unwrap();
    }

    // Read from position 5
    let options = StreamReadOptions::new(stream_name).with_position(5);
    let messages = client.get_stream_messages(options).await.unwrap();

    assert_eq!(messages.len(), 5); // Messages 5-9
    assert_eq!(messages[0].position, 5);
    assert_eq!(messages[0].data["sequence"], 5);
}

#[tokio::test]
async fn test_get_stream_messages_with_batch_size() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-stream-read-3";

    // Write 10 messages
    for i in 0..10 {
        let msg = WriteMessage::new(Uuid::new_v4(), stream_name, "TestEvent")
            .with_data(json!({ "sequence": i }));
        client.write_message(msg).await.unwrap();
    }

    // Read with batch size of 3
    let options = StreamReadOptions::new(stream_name).with_batch_size(3);
    let messages = client.get_stream_messages(options).await.unwrap();

    assert_eq!(messages.len(), 3);
}

#[tokio::test]
async fn test_get_stream_messages_metadata() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-stream-metadata";
    let correlation_id = "corr-123";

    let msg = WriteMessage::new(Uuid::new_v4(), stream_name, "TestEvent")
        .with_data(json!({ "value": 42 }))
        .with_metadata(json!({ "correlation_id": correlation_id }));

    client.write_message(msg).await.unwrap();

    let options = StreamReadOptions::new(stream_name);
    let messages = client.get_stream_messages(options).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].correlation_id(), Some(correlation_id));
}

// ============================================================================
// get_category_messages tests
// ============================================================================

#[tokio::test]
async fn test_get_category_messages_basic() {
    setup_test!(_docker, _container, client);

    let category = "testcategory1";

    // Write messages to different streams in the same category
    for i in 0..3 {
        let stream_name = format!("{}-{}", category, i);
        let msg = WriteMessage::new(Uuid::new_v4(), &stream_name, "TestEvent")
            .with_data(json!({ "stream_id": i }));
        client.write_message(msg).await.unwrap();
    }

    // Read category messages
    let options = CategoryReadOptions::new(category);
    let messages = client.get_category_messages(options).await.unwrap();

    // Should get all 3 messages
    assert_eq!(messages.len(), 3);
}

#[tokio::test]
async fn test_get_category_messages_with_batch_size() {
    setup_test!(_docker, _container, client);

    let category = "testcategory2";

    // Write messages to different streams
    for i in 0..5 {
        let stream_name = format!("{}-{}", category, i);
        let msg = WriteMessage::new(Uuid::new_v4(), &stream_name, "TestEvent")
            .with_data(json!({ "stream_id": i }));
        client.write_message(msg).await.unwrap();
    }

    // Read with batch size
    let options = CategoryReadOptions::new(category).with_batch_size(2);
    let messages = client.get_category_messages(options).await.unwrap();

    assert!(messages.len() <= 2);
}

#[tokio::test]
async fn test_get_category_messages_ordering() {
    setup_test!(_docker, _container, client);

    let category = "testcategoryorder";

    // Write messages to different streams
    for i in 0..3 {
        let stream_name = format!("{}-{}", category, i);
        let msg = WriteMessage::new(Uuid::new_v4(), &stream_name, "TestEvent")
            .with_data(json!({ "stream_id": i }));
        client.write_message(msg).await.unwrap();
    }

    // Read category messages
    let options = CategoryReadOptions::new(category);
    let messages = client.get_category_messages(options).await.unwrap();

    // Messages should be ordered by global_position
    for i in 1..messages.len() {
        assert!(
            messages[i].global_position > messages[i - 1].global_position,
            "Messages should be ordered by global position"
        );
    }
}

// ============================================================================
// get_last_stream_message tests
// ============================================================================

#[tokio::test]
async fn test_get_last_stream_message_empty() {
    setup_test!(_docker, _container, client);

    let result = client
        .get_last_stream_message("nonexistent-stream", None)
        .await
        .unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_last_stream_message_single() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-last-1";
    let msg_id = Uuid::new_v4();

    let msg = WriteMessage::new(msg_id, stream_name, "TestEvent")
        .with_data(json!({ "value": 42 }));
    client.write_message(msg).await.unwrap();

    let last_msg = client
        .get_last_stream_message(stream_name, None)
        .await
        .unwrap()
        .expect("Should have a message");

    assert_eq!(last_msg.id, msg_id);
    assert_eq!(last_msg.position, 0);
}

#[tokio::test]
async fn test_get_last_stream_message_multiple() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-last-2";

    // Write multiple messages
    let mut last_id = Uuid::nil();
    for i in 0..5 {
        let msg_id = Uuid::new_v4();
        let msg = WriteMessage::new(msg_id, stream_name, "TestEvent")
            .with_data(json!({ "sequence": i }));
        client.write_message(msg).await.unwrap();
        if i == 4 {
            last_id = msg_id;
        }
    }

    let last_msg = client
        .get_last_stream_message(stream_name, None)
        .await
        .unwrap()
        .expect("Should have a message");

    assert_eq!(last_msg.id, last_id);
    assert_eq!(last_msg.position, 4);
    assert_eq!(last_msg.data["sequence"], 4);
}

#[tokio::test]
async fn test_get_last_stream_message_by_type() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-last-type";

    // Write messages of different types
    let deposited_id = Uuid::new_v4();
    let msg1 = WriteMessage::new(Uuid::new_v4(), stream_name, "Opened");
    client.write_message(msg1).await.unwrap();

    let msg2 = WriteMessage::new(deposited_id, stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    client.write_message(msg2).await.unwrap();

    let msg3 = WriteMessage::new(Uuid::new_v4(), stream_name, "Withdrawn")
        .with_data(json!({ "amount": 50 }));
    client.write_message(msg3).await.unwrap();

    let msg4 = WriteMessage::new(Uuid::new_v4(), stream_name, "Deposited")
        .with_data(json!({ "amount": 200 }));
    client.write_message(msg4).await.unwrap();

    // Get last "Deposited" message
    let last_deposited = client
        .get_last_stream_message(stream_name, Some("Deposited"))
        .await
        .unwrap()
        .expect("Should have a Deposited message");

    assert_eq!(last_deposited.message_type, "Deposited");
    assert_eq!(last_deposited.data["amount"], 200);
}

// ============================================================================
// stream_version tests
// ============================================================================

#[tokio::test]
async fn test_stream_version_nonexistent() {
    setup_test!(_docker, _container, client);

    let version = client.stream_version("nonexistent-stream").await.unwrap();
    assert!(version.is_none());
}

#[tokio::test]
async fn test_stream_version_single_message() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-version-1";

    let msg = WriteMessage::new(Uuid::new_v4(), stream_name, "TestEvent");
    client.write_message(msg).await.unwrap();

    let version = client
        .stream_version(stream_name)
        .await
        .unwrap()
        .expect("Should have a version");

    assert_eq!(version, 0);
}

#[tokio::test]
async fn test_stream_version_multiple_messages() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-version-2";

    // Write 10 messages
    for _ in 0..10 {
        let msg = WriteMessage::new(Uuid::new_v4(), stream_name, "TestEvent");
        client.write_message(msg).await.unwrap();
    }

    let version = client
        .stream_version(stream_name)
        .await
        .unwrap()
        .expect("Should have a version");

    assert_eq!(version, 9); // 10 messages = positions 0-9, version is 9
}

#[tokio::test]
async fn test_stream_version_after_writes() {
    setup_test!(_docker, _container, client);

    let stream_name = "test-version-progression";

    // Initially no version
    let v0 = client.stream_version(stream_name).await.unwrap();
    assert!(v0.is_none());

    // After first write
    let msg1 = WriteMessage::new(Uuid::new_v4(), stream_name, "Event1");
    client.write_message(msg1).await.unwrap();
    let v1 = client.stream_version(stream_name).await.unwrap().unwrap();
    assert_eq!(v1, 0);

    // After second write
    let msg2 = WriteMessage::new(Uuid::new_v4(), stream_name, "Event2");
    client.write_message(msg2).await.unwrap();
    let v2 = client.stream_version(stream_name).await.unwrap().unwrap();
    assert_eq!(v2, 1);

    // After third write
    let msg3 = WriteMessage::new(Uuid::new_v4(), stream_name, "Event3");
    client.write_message(msg3).await.unwrap();
    let v3 = client.stream_version(stream_name).await.unwrap().unwrap();
    assert_eq!(v3, 2);
}
