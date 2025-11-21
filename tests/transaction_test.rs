mod common;

use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::message_db::types::WriteMessage;
use rust2::message_db::operations::StreamReadOptions;
use serde_json::json;
use testcontainers::clients::Cli;
use uuid::Uuid;

#[tokio::test]
async fn test_transaction_commit() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream name for this test
    let stream_name = format!("test-account-{}", Uuid::new_v4());

    // Begin transaction
    let mut txn = client.begin_transaction().await.unwrap();

    // Write two messages in the transaction
    let msg1 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    let msg2 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Withdrawn")
        .with_data(json!({ "amount": 50 }));

    let pos1 = txn.write_message(msg1).await.unwrap();
    let pos2 = txn.write_message(msg2).await.unwrap();

    assert_eq!(pos1, 0);
    assert_eq!(pos2, 1);

    // Commit the transaction
    txn.commit().await.unwrap();

    // Verify messages were written
    let messages = client
        .get_stream_messages(StreamReadOptions::new(&stream_name))
        .await
        .unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].message_type, "Deposited");
    assert_eq!(messages[1].message_type, "Withdrawn");
}

#[tokio::test]
async fn test_transaction_rollback() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream name for this test
    let stream_name = format!("test-account-{}", Uuid::new_v4());

    // Begin transaction
    let mut txn = client.begin_transaction().await.unwrap();

    // Write a message in the transaction
    let msg = WriteMessage::new(Uuid::new_v4(), &stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));

    txn.write_message(msg).await.unwrap();

    // Rollback the transaction
    txn.rollback().await.unwrap();

    // Verify message was NOT written
    let messages = client
        .get_stream_messages(StreamReadOptions::new(&stream_name))
        .await
        .unwrap();

    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_transaction_atomic_multi_write() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream names for this test (money transfer scenario)
    let stream1 = format!("test-account-{}", Uuid::new_v4());
    let stream2 = format!("test-account-{}", Uuid::new_v4());

    // Begin transaction
    let mut txn = client.begin_transaction().await.unwrap();

    // Debit from account 1
    let msg1 = WriteMessage::new(Uuid::new_v4(), &stream1, "Withdrawn")
        .with_data(json!({ "amount": 100 }));

    // Credit to account 2
    let msg2 = WriteMessage::new(Uuid::new_v4(), &stream2, "Deposited")
        .with_data(json!({ "amount": 100 }));

    txn.write_message(msg1).await.unwrap();
    txn.write_message(msg2).await.unwrap();

    // Commit the transaction
    txn.commit().await.unwrap();

    // Verify both messages were written
    let messages1 = client
        .get_stream_messages(StreamReadOptions::new(&stream1))
        .await
        .unwrap();
    let messages2 = client
        .get_stream_messages(StreamReadOptions::new(&stream2))
        .await
        .unwrap();

    assert_eq!(messages1.len(), 1);
    assert_eq!(messages1[0].message_type, "Withdrawn");
    assert_eq!(messages2.len(), 1);
    assert_eq!(messages2[0].message_type, "Deposited");
}

#[tokio::test]
async fn test_transaction_concurrency_error() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream name for this test
    let stream_name = format!("test-account-{}", Uuid::new_v4());

    // Write initial message outside of transaction
    let initial_msg = WriteMessage::new(Uuid::new_v4(), &stream_name, "Opened")
        .with_data(json!({ "balance": 1000 }));
    client.write_message(initial_msg).await.unwrap();

    // Begin transaction with expected version
    let mut txn = client.begin_transaction().await.unwrap();

    // Try to write with wrong expected version
    let msg = WriteMessage::new(Uuid::new_v4(), &stream_name, "Withdrawn")
        .with_data(json!({ "amount": 50 }))
        .with_expected_version(10); // Wrong version - stream is at 0

    let result = txn.write_message(msg).await;

    // Should get a concurrency error
    assert!(result.is_err());
    match result {
        Err(rust2::message_db::Error::ConcurrencyError { .. }) => {
            // Expected error
        }
        _ => panic!("Expected ConcurrencyError"),
    }

    // Rollback the transaction
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn test_transaction_idempotent_write_aborts() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream name and message ID
    let stream_name = format!("test-account-{}", Uuid::new_v4());
    let msg_id = Uuid::new_v4();

    // Write message in first transaction
    let mut txn1 = client.begin_transaction().await.unwrap();
    let msg1 = WriteMessage::new(msg_id, &stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    txn1.write_message(msg1).await.unwrap();
    txn1.commit().await.unwrap();

    // Try to write same message ID in second transaction
    // This should fail because duplicate key error aborts the transaction
    let mut txn2 = client.begin_transaction().await.unwrap();
    let msg2 = WriteMessage::new(msg_id, &stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    let result = txn2.write_message(msg2).await;

    // Should get an error about duplicate key
    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("Duplicate message ID") || err_msg.contains("idempotent"));

    // Must rollback the aborted transaction
    txn2.rollback().await.unwrap();

    // Verify only one message exists
    let messages = client
        .get_stream_messages(StreamReadOptions::new(&stream_name))
        .await
        .unwrap();
    assert_eq!(messages.len(), 1);
}

#[tokio::test]
async fn test_transaction_read_within_transaction() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream name for this test
    let stream_name = format!("test-account-{}", Uuid::new_v4());

    // Write initial messages outside transaction
    let msg1 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    let msg2 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Deposited")
        .with_data(json!({ "amount": 50 }));
    client.write_message(msg1).await.unwrap();
    client.write_message(msg2).await.unwrap();

    // Begin transaction and read messages
    let mut txn = client.begin_transaction().await.unwrap();

    let messages = txn
        .get_stream_messages(StreamReadOptions::new(&stream_name))
        .await
        .unwrap();

    assert_eq!(messages.len(), 2);

    // Get stream version within transaction
    let version = txn.stream_version(&stream_name).await.unwrap();
    assert_eq!(version, Some(1)); // Last message position is 1 (0-based)

    // Write another message with correct expected version
    let msg3 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Withdrawn")
        .with_data(json!({ "amount": 30 }))
        .with_expected_version(1);

    txn.write_message(msg3).await.unwrap();
    txn.commit().await.unwrap();

    // Verify all three messages exist
    let final_messages = client
        .get_stream_messages(StreamReadOptions::new(&stream_name))
        .await
        .unwrap();
    assert_eq!(final_messages.len(), 3);
}

#[tokio::test]
async fn test_transaction_get_last_message() {
    // Start Message DB container
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host = "127.0.0.1";
    let port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string(host, port);

    // Create client
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create unique stream name for this test
    let stream_name = format!("test-account-{}", Uuid::new_v4());

    // Write initial messages
    let msg1 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Deposited")
        .with_data(json!({ "amount": 100 }));
    let msg2 = WriteMessage::new(Uuid::new_v4(), &stream_name, "Withdrawn")
        .with_data(json!({ "amount": 50 }));
    client.write_message(msg1).await.unwrap();
    client.write_message(msg2).await.unwrap();

    // Begin transaction and get last message
    let txn = client.begin_transaction().await.unwrap();

    let last_msg = txn
        .get_last_stream_message(&stream_name, None)
        .await
        .unwrap();

    assert!(last_msg.is_some());
    let last_msg = last_msg.unwrap();
    assert_eq!(last_msg.message_type, "Withdrawn");
    assert_eq!(last_msg.position, 1);

    txn.commit().await.unwrap();
}
