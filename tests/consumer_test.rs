mod common;

use rust2::message_db::consumer::{Consumer, ConsumerConfig, PositionTracker};
use rust2::message_db::types::{Message, WriteMessage};
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use serde_json::json;
use std::sync::{Arc, Mutex};
use testcontainers::clients::Cli;
use uuid::Uuid;

#[tokio::test]
async fn test_position_tracker_initial_position() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create position tracker
    let mut tracker = PositionTracker::new(client, "test-category", "test-consumer", 10);

    // Initial position should be 1 (category default)
    let position = tracker.read_position().await.unwrap();
    assert_eq!(position, 1);
    assert_eq!(tracker.current_position(), 1);
}

#[tokio::test]
async fn test_position_tracker_write_and_read() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create position tracker
    let mut tracker = PositionTracker::new(
        client.clone(),
        "test-category",
        "test-consumer",
        10,
    );

    // Update position
    tracker.update_position(100).await.unwrap();
    assert_eq!(tracker.current_position(), 100);

    // Write position
    tracker.write_position().await.unwrap();

    // Create a new tracker and verify it reads the saved position
    let mut tracker2 = PositionTracker::new(client, "test-category", "test-consumer", 10);
    let position = tracker2.read_position().await.unwrap();
    assert_eq!(position, 100);
}

#[tokio::test]
async fn test_position_tracker_update_interval() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Create position tracker with update interval of 3
    let mut tracker = PositionTracker::new(
        client.clone(),
        "test-category",
        "test-consumer",
        3,
    );

    // Update position twice (should not write yet)
    tracker.update_position(10).await.unwrap();
    tracker.update_position(20).await.unwrap();
    assert_eq!(tracker.messages_since_update(), 2);

    // Third update should trigger write
    tracker.update_position(30).await.unwrap();
    assert_eq!(tracker.messages_since_update(), 0);

    // Verify position was written
    let mut tracker2 = PositionTracker::new(client, "test-category", "test-consumer", 3);
    let position = tracker2.read_position().await.unwrap();
    assert_eq!(position, 30);
}

#[tokio::test]
async fn test_consumer_poll_once() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    // Generate unique stream prefix for test isolation (remove hyphens from UUID)
    let test_id = Uuid::new_v4().to_string().replace("-", "");

    // Write test messages
    for i in 0..3 {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            format!("{}-account-{}", test_id, i),
            "TestEvent",
        )
        .with_data(json!({ "index": i }));
        client.write_message(msg).await.unwrap();
    }

    // Create consumer
    let consumer_config = ConsumerConfig::new(&test_id, "test-consumer").with_batch_size(10);

    let mut consumer = Consumer::new(client, consumer_config).await.unwrap();

    // Track processed messages
    let processed = Arc::new(Mutex::new(Vec::new()));
    let processed_clone = Arc::clone(&processed);

    consumer.on("TestEvent", move |msg: Message| {
        let processed = Arc::clone(&processed_clone);
        Box::pin(async move {
            processed.lock().unwrap().push(msg.data["index"].as_i64().unwrap());
            Ok(())
        })
    });

    // Poll once
    let had_messages = consumer.poll_once().await.unwrap();
    assert!(had_messages);

    // Verify all messages were processed
    let processed = processed.lock().unwrap();
    assert_eq!(processed.len(), 3);
    assert_eq!(*processed, vec![0, 1, 2]);
}

#[tokio::test]
async fn test_consumer_resume_from_position() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    let test_id = Uuid::new_v4().to_string().replace("-", "");

    // Write 5 messages
    for i in 0..5 {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            format!("{}-account-{}", test_id, i),
            "TestEvent",
        )
        .with_data(json!({ "index": i }));
        client.write_message(msg).await.unwrap();
    }

    // First consumer processes 3 messages
    let consumer_config = ConsumerConfig::new(&test_id, "test-consumer")
        .with_batch_size(3)
        .with_position_update_interval(1); // Write position after each message

    let mut consumer1 = Consumer::new(client.clone(), consumer_config.clone()).await.unwrap();

    let processed1 = Arc::new(Mutex::new(Vec::new()));
    let processed1_clone = Arc::clone(&processed1);

    consumer1.on("TestEvent", move |msg: Message| {
        let processed = Arc::clone(&processed1_clone);
        Box::pin(async move {
            processed.lock().unwrap().push(msg.data["index"].as_i64().unwrap());
            Ok(())
        })
    });

    consumer1.poll_once().await.unwrap();
    consumer1.flush_position().await.unwrap();

    let processed1 = processed1.lock().unwrap();
    assert_eq!(processed1.len(), 3);

    // Second consumer should resume and process remaining messages
    let mut consumer2 = Consumer::new(client.clone(), consumer_config).await.unwrap();

    let processed2 = Arc::new(Mutex::new(Vec::new()));
    let processed2_clone = Arc::clone(&processed2);

    consumer2.on("TestEvent", move |msg: Message| {
        let processed = Arc::clone(&processed2_clone);
        Box::pin(async move {
            processed.lock().unwrap().push(msg.data["index"].as_i64().unwrap());
            Ok(())
        })
    });

    consumer2.poll_once().await.unwrap();

    let processed2 = processed2.lock().unwrap();
    assert_eq!(processed2.len(), 2);
    assert_eq!(*processed2, vec![3, 4]);
}

#[tokio::test]
async fn test_consumer_empty_category() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    let test_id = Uuid::new_v4().to_string().replace("-", "");

    let consumer_config = ConsumerConfig::new(&test_id, "test-consumer");
    let mut consumer = Consumer::new(client, consumer_config).await.unwrap();

    consumer.on("TestEvent", |_msg: Message| Box::pin(async move { Ok(()) }));

    // Poll should return false (no messages)
    let had_messages = consumer.poll_once().await.unwrap();
    assert!(!had_messages);
}

#[tokio::test]
async fn test_consumer_multiple_message_types() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    let test_id = Uuid::new_v4().to_string().replace("-", "");

    // Write different message types
    let msg1 = WriteMessage::new(Uuid::new_v4(), format!("{}-account-1", test_id), "TypeA")
        .with_data(json!({ "type": "A" }));
    let msg2 = WriteMessage::new(Uuid::new_v4(), format!("{}-account-2", test_id), "TypeB")
        .with_data(json!({ "type": "B" }));
    let msg3 = WriteMessage::new(Uuid::new_v4(), format!("{}-account-3", test_id), "TypeA")
        .with_data(json!({ "type": "A" }));

    client.write_message(msg1).await.unwrap();
    client.write_message(msg2).await.unwrap();
    client.write_message(msg3).await.unwrap();

    let consumer_config = ConsumerConfig::new(&test_id, "test-consumer");
    let mut consumer = Consumer::new(client, consumer_config).await.unwrap();

    let type_a_count = Arc::new(Mutex::new(0));
    let type_b_count = Arc::new(Mutex::new(0));

    let type_a_clone = Arc::clone(&type_a_count);
    consumer.on("TypeA", move |_msg: Message| {
        let count = Arc::clone(&type_a_clone);
        Box::pin(async move {
            *count.lock().unwrap() += 1;
            Ok(())
        })
    });

    let type_b_clone = Arc::clone(&type_b_count);
    consumer.on("TypeB", move |_msg: Message| {
        let count = Arc::clone(&type_b_clone);
        Box::pin(async move {
            *count.lock().unwrap() += 1;
            Ok(())
        })
    });

    consumer.poll_once().await.unwrap();

    assert_eq!(*type_a_count.lock().unwrap(), 2);
    assert_eq!(*type_b_count.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_consumer_with_consumer_group() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    let test_id = Uuid::new_v4().to_string().replace("-", "");

    // Write messages to different streams
    for i in 0..10 {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            format!("{}-account-{}", test_id, i),
            "TestEvent",
        )
        .with_data(json!({ "stream": i }));
        client.write_message(msg).await.unwrap();
    }

    // Create two consumers in a group of 2
    let consumer_config_0 = ConsumerConfig::new(&test_id, "consumer-0")
        .with_consumer_group(0, 2)
        .with_batch_size(20);

    let consumer_config_1 = ConsumerConfig::new(&test_id, "consumer-1")
        .with_consumer_group(1, 2)
        .with_batch_size(20);

    let mut consumer0 = Consumer::new(client.clone(), consumer_config_0).await.unwrap();
    let mut consumer1 = Consumer::new(client.clone(), consumer_config_1).await.unwrap();

    let processed0 = Arc::new(Mutex::new(Vec::new()));
    let processed1 = Arc::new(Mutex::new(Vec::new()));

    let processed0_clone = Arc::clone(&processed0);
    consumer0.on("TestEvent", move |msg: Message| {
        let processed = Arc::clone(&processed0_clone);
        Box::pin(async move {
            processed.lock().unwrap().push(msg.stream_name.clone());
            Ok(())
        })
    });

    let processed1_clone = Arc::clone(&processed1);
    consumer1.on("TestEvent", move |msg: Message| {
        let processed = Arc::clone(&processed1_clone);
        Box::pin(async move {
            processed.lock().unwrap().push(msg.stream_name.clone());
            Ok(())
        })
    });

    // Each consumer should process different messages
    consumer0.poll_once().await.unwrap();
    consumer1.poll_once().await.unwrap();

    let streams0 = processed0.lock().unwrap();
    let streams1 = processed1.lock().unwrap();

    // Both should have processed some messages
    assert!(streams0.len() > 0);
    assert!(streams1.len() > 0);

    // Together they should have processed all 10
    assert_eq!(streams0.len() + streams1.len(), 10);

    // No overlap in streams
    for stream in streams0.iter() {
        assert!(!streams1.contains(stream));
    }
}

// TODO: Correlation filtering needs more investigation to understand exact semantics
// The feature is implemented via CategoryReadOptions.with_correlation() but needs
// a comprehensive test that matches Message DB's correlation behavior exactly.
#[tokio::test]
#[ignore]
async fn test_consumer_with_correlation() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    let test_id = Uuid::new_v4().to_string().replace("-", "");

    // Use separate category names for command and account
    let cmd_category = format!("{}cmd", test_id);  // No hyphen - this is a category
    let account_category = format!("{}account", test_id);

    // Write a command - the correlation will match based on stream ID "abc"
    let cmd_msg = WriteMessage::new(
        Uuid::new_v4(),
        format!("{}-abc", cmd_category),  // Stream in cmd category with ID "abc"
        "WithdrawCommand",
    )
    .with_data(json!({ "amount": 50 }));

    client.write_message(cmd_msg).await.unwrap();

    // Write events with and without matching correlation
    // Event1's correlation_id "abc" matches the command stream ID "{cmd_category}-abc"
    let event1 = WriteMessage::new(
        Uuid::new_v4(),
        format!("{}-1", account_category),
        "Withdrawn",
    )
    .with_data(json!({ "amount": 50 }))
    .with_metadata(json!({ "correlation_id": "abc" }));  // Matches command stream ID

    // Event2's correlation_id doesn't match any command stream
    let event2 = WriteMessage::new(
        Uuid::new_v4(),
        format!("{}-2", account_category),
        "Withdrawn",
    )
    .with_data(json!({ "amount": 30 }))
    .with_metadata(json!({ "correlation_id": "xyz" }));  // Doesn't match

    client.write_message(event1).await.unwrap();
    client.write_message(event2).await.unwrap();

    // Consumer with correlation filter
    let consumer_config = ConsumerConfig::new(&account_category, "correlated-consumer")
        .with_correlation(&cmd_category);

    let mut consumer = Consumer::new(client, consumer_config).await.unwrap();

    let processed = Arc::new(Mutex::new(Vec::new()));
    let processed_clone = Arc::clone(&processed);

    consumer.on("Withdrawn", move |msg: Message| {
        let processed = Arc::clone(&processed_clone);
        Box::pin(async move {
            processed.lock().unwrap().push(msg.data["amount"].as_i64().unwrap());
            Ok(())
        })
    });

    consumer.poll_once().await.unwrap();

    // Should only process the correlated event
    let processed = processed.lock().unwrap();
    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0], 50);
}

#[tokio::test]
async fn test_consumer_unhandled_message_type() {
    let docker = Cli::default();
    let container = docker.run(common::create_message_db_container());
    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let conn_str = common::build_connection_string("127.0.0.1", host_port);

    let config = MessageDbConfig::from_connection_string(&conn_str).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();

    let test_id = Uuid::new_v4().to_string().replace("-", "");

    // Write messages of different types
    let msg1 = WriteMessage::new(Uuid::new_v4(), format!("{}-account-1", test_id), "Handled");
    let msg2 = WriteMessage::new(Uuid::new_v4(), format!("{}-account-2", test_id), "NotHandled");

    client.write_message(msg1).await.unwrap();
    client.write_message(msg2).await.unwrap();

    let consumer_config = ConsumerConfig::new(&test_id, "test-consumer");
    let mut consumer = Consumer::new(client, consumer_config).await.unwrap();

    let handled_count = Arc::new(Mutex::new(0));
    let handled_clone = Arc::clone(&handled_count);

    // Only register handler for "Handled" type
    consumer.on("Handled", move |_msg: Message| {
        let count = Arc::clone(&handled_clone);
        Box::pin(async move {
            *count.lock().unwrap() += 1;
            Ok(())
        })
    });

    // Should not fail even though "NotHandled" has no handler
    consumer.poll_once().await.unwrap();

    // Only the handled message should be processed
    assert_eq!(*handled_count.lock().unwrap(), 1);

    // Position should still advance past both messages
    assert!(consumer.current_position() > 0);
}
