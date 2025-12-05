mod common;

use chrono::Utc;
use rust2::llm::agent::{
    events::*,
    store::ThreadStore,
};
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use serde_json::json;
use testcontainers::clients::Cli;

// Macro to set up test environment
// Note: This keeps _docker and _container alive for the duration of the test
macro_rules! setup_test {
    ($docker:ident, $container:ident, $store:ident) => {
        let $docker = Cli::default();
        let $container = $docker.run(common::create_message_db_container());

        // Give the container a moment to fully initialize
        // Message DB needs time to create its functions after PostgreSQL is ready
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let host_port = $container.get_host_port_ipv4(common::POSTGRES_PORT);
        let connection_string = common::build_connection_string("127.0.0.1", host_port);
        let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
        let client = MessageDbClient::new(config).await.unwrap();
        let $store = ThreadStore::new(client);
    };
}

// =============================================================================
// Basic Operations Tests
// =============================================================================

#[tokio::test]
async fn test_read_empty_thread() {
    setup_test!(_docker, _container, store);

    let events = store
        .read_thread_events("thread-empty-123")
        .await
        .expect("Should successfully read empty thread");

    assert_eq!(events.len(), 0, "Empty thread should return no events");
}

#[tokio::test]
async fn test_append_single_event() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();
    let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "Hello, world!".to_string(),
        timestamp: now,
    });

    let version = store
        .append_event("thread-single-456", event.clone(), None, None)
        .await
        .expect("Should successfully append event");

    assert_eq!(version, 0, "First event should be at position 0");

    // Read back and verify
    let events = store
        .read_thread_events("thread-single-456")
        .await
        .expect("Should read events");

    assert_eq!(events.len(), 1, "Should have one event");

    match &events[0] {
        ThreadEvent::UserMessageReceived(data) => {
            assert_eq!(data.message, "Hello, world!");
        }
        _ => panic!("Wrong event type"),
    }
}

#[tokio::test]
async fn test_append_multiple_events_sequentially() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    // Append first event
    let event1 = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "First message".to_string(),
        timestamp: now,
    });
    let v1 = store
        .append_event("thread-multi-789", event1, None, None)
        .await
        .expect("Should append first event");
    assert_eq!(v1, 0);

    // Append second event
    let event2 = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
        iteration: 1,
        timestamp: now,
    });
    let v2 = store
        .append_event("thread-multi-789", event2, None, None)
        .await
        .expect("Should append second event");
    assert_eq!(v2, 1);

    // Append third event
    let event3 = ThreadEvent::LlmCallStarted(LlmCallStartedData {
        provider: "claude".to_string(),
        model: "sonnet".to_string(),
        message_count: 1,
        timestamp: now,
    });
    let v3 = store
        .append_event("thread-multi-789", event3, None, None)
        .await
        .expect("Should append third event");
    assert_eq!(v3, 2);

    // Read back and verify order
    let events = store
        .read_thread_events("thread-multi-789")
        .await
        .expect("Should read events");

    assert_eq!(events.len(), 3, "Should have three events");

    match &events[0] {
        ThreadEvent::UserMessageReceived(data) => {
            assert_eq!(data.message, "First message");
        }
        _ => panic!("Wrong event type at position 0"),
    }

    match &events[1] {
        ThreadEvent::AgentIterationStarted(data) => {
            assert_eq!(data.iteration, 1);
        }
        _ => panic!("Wrong event type at position 1"),
    }

    match &events[2] {
        ThreadEvent::LlmCallStarted(data) => {
            assert_eq!(data.provider, "claude");
        }
        _ => panic!("Wrong event type at position 2"),
    }
}

// =============================================================================
// Batch Append Tests
// =============================================================================

#[tokio::test]
async fn test_append_events_batch() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    let events = vec![
        ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Hello".to_string(),
            timestamp: now,
        }),
        ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 1,
            timestamp: now,
        }),
        ThreadEvent::LlmCallStarted(LlmCallStartedData {
            provider: "claude".to_string(),
            model: "sonnet".to_string(),
            message_count: 1,
            timestamp: now,
        }),
    ];

    let final_version = store
        .append_events("thread-batch-001", events, None, None)
        .await
        .expect("Should append batch");

    assert_eq!(final_version, 2, "Final version should be 2 (0-indexed)");

    // Read back and verify
    let read_events = store
        .read_thread_events("thread-batch-001")
        .await
        .expect("Should read events");

    assert_eq!(read_events.len(), 3, "Should have all three events");
}

#[tokio::test]
async fn test_append_empty_batch_fails() {
    setup_test!(_docker, _container, store);

    let events: Vec<ThreadEvent> = vec![];

    let result = store
        .append_events("thread-empty-batch", events, None, None)
        .await;

    assert!(result.is_err(), "Should fail with empty event list");
}

// =============================================================================
// Optimistic Concurrency Tests
// =============================================================================

#[tokio::test]
async fn test_optimistic_concurrency_success() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    // Write first event with no expected version
    let event1 = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "First".to_string(),
        timestamp: now,
    });
    let v1 = store
        .append_event("thread-occ-success", event1, None, None)
        .await
        .expect("Should append first event");

    // Write second event with correct expected version
    let event2 = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
        iteration: 1,
        timestamp: now,
    });
    let v2 = store
        .append_event("thread-occ-success", event2, Some(v1), None)
        .await
        .expect("Should append with correct expected version");

    assert_eq!(v2, 1, "Second event should be at position 1");

    // Write third event with correct expected version
    let event3 = ThreadEvent::AgentCompleted(AgentCompletedData {
        total_iterations: 1,
        final_response: "Done".to_string(),
        timestamp: now,
    });
    let v3 = store
        .append_event("thread-occ-success", event3, Some(v2), None)
        .await
        .expect("Should append with correct expected version");

    assert_eq!(v3, 2, "Third event should be at position 2");
}

#[tokio::test]
async fn test_optimistic_concurrency_conflict() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    // Write first event
    let event1 = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "First".to_string(),
        timestamp: now,
    });
    store
        .append_event("thread-occ-conflict", event1, None, None)
        .await
        .expect("Should append first event");

    // Write second event
    let event2 = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
        iteration: 1,
        timestamp: now,
    });
    store
        .append_event("thread-occ-conflict", event2, None, None)
        .await
        .expect("Should append second event");

    // Try to write third event with wrong expected version (simulating stale read)
    let event3 = ThreadEvent::AgentCompleted(AgentCompletedData {
        total_iterations: 1,
        final_response: "Done".to_string(),
        timestamp: now,
    });
    let result = store
        .append_event("thread-occ-conflict", event3, Some(0), None)
        .await;

    assert!(
        result.is_err(),
        "Should fail with wrong expected version (expected conflict)"
    );
}

#[tokio::test]
async fn test_batch_optimistic_concurrency() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    // Write initial event
    let event0 = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "Initial".to_string(),
        timestamp: now,
    });
    let v0 = store
        .append_event("thread-batch-occ", event0, None, None)
        .await
        .expect("Should append initial event");

    // Batch append with correct expected version
    let events = vec![
        ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 1,
            timestamp: now,
        }),
        ThreadEvent::LlmCallStarted(LlmCallStartedData {
            provider: "claude".to_string(),
            model: "sonnet".to_string(),
            message_count: 2,
            timestamp: now,
        }),
    ];

    let final_v = store
        .append_events("thread-batch-occ", events, Some(v0), None)
        .await
        .expect("Should append batch with correct expected version");

    assert_eq!(final_v, 2, "Final version should be 2");
}

// =============================================================================
// Stream Name and Metadata Tests
// =============================================================================

#[tokio::test]
async fn test_stream_name_format() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();
    let thread_id = "550e8400-e29b-41d4-a716-446655440000";

    let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "Test".to_string(),
        timestamp: now,
    });

    store
        .append_event(thread_id, event, None, None)
        .await
        .expect("Should append event");

    // The stream should be readable with the same thread_id
    let events = store
        .read_thread_events(thread_id)
        .await
        .expect("Should read events");

    assert_eq!(events.len(), 1, "Should have one event");
}

#[tokio::test]
async fn test_event_with_metadata() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    let metadata = json!({
        "correlation_id": "corr-123",
        "user_id": "user-456",
        "client_ip": "192.168.1.1"
    });

    let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "Hello with metadata".to_string(),
        timestamp: now,
    });

    store
        .append_event("thread-metadata", event, None, Some(metadata))
        .await
        .expect("Should append event with metadata");

    // Read back - metadata is stored but not returned in ThreadEvent
    // (it's in the MessageDB metadata field, not the event data)
    let events = store
        .read_thread_events("thread-metadata")
        .await
        .expect("Should read events");

    assert_eq!(events.len(), 1, "Should have one event");
}

// =============================================================================
// Stream Version Tests
// =============================================================================

#[tokio::test]
async fn test_get_stream_version_empty() {
    setup_test!(_docker, _container, store);

    let version = store
        .get_stream_version("thread-version-empty")
        .await
        .expect("Should get version");

    assert_eq!(version, None, "Empty stream should have no version");
}

#[tokio::test]
async fn test_get_stream_version_after_writes() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    // Write first event
    let event1 = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "First".to_string(),
        timestamp: now,
    });
    store
        .append_event("thread-version-writes", event1, None, None)
        .await
        .expect("Should append event");

    let v1 = store
        .get_stream_version("thread-version-writes")
        .await
        .expect("Should get version");
    assert_eq!(v1, Some(0), "Version should be 0 after first write");

    // Write second event
    let event2 = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
        iteration: 1,
        timestamp: now,
    });
    store
        .append_event("thread-version-writes", event2, None, None)
        .await
        .expect("Should append event");

    let v2 = store
        .get_stream_version("thread-version-writes")
        .await
        .expect("Should get version");
    assert_eq!(v2, Some(1), "Version should be 1 after second write");
}

// =============================================================================
// Event Type Coverage Tests
// =============================================================================

#[tokio::test]
async fn test_all_event_types_roundtrip() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    let events = vec![
        ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Hello".to_string(),
            timestamp: now,
        }),
        ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 1,
            timestamp: now,
        }),
        ThreadEvent::LlmCallStarted(LlmCallStartedData {
            provider: "claude".to_string(),
            model: "sonnet-4-5".to_string(),
            message_count: 1,
            timestamp: now,
        }),
        ThreadEvent::LlmContentDelta(LlmContentDeltaData {
            content_block_index: 0,
            delta_type: "text".to_string(),
            text: "The".to_string(),
            timestamp: now,
        }),
        ThreadEvent::LlmToolUseStarted(LlmToolUseStartedData {
            tool_use_id: "toolu_123".to_string(),
            content_block_index: 1,
            name: "get_weather".to_string(),
            timestamp: now,
        }),
        ThreadEvent::LlmToolUseDelta(LlmToolUseDeltaData {
            tool_use_id: "toolu_123".to_string(),
            partial_json: r#"{"location":"#.to_string(),
            timestamp: now,
        }),
        ThreadEvent::LlmToolUseCompleted(LlmToolUseCompletedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "Tokyo"}),
            timestamp: now,
        }),
        ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
            stop_reason: "tool_use".to_string(),
            content_blocks: vec![
                ContentBlockData::Text {
                    text: "Let me check".to_string(),
                },
                ContentBlockData::ToolUse {
                    id: "toolu_123".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "Tokyo"}),
                },
            ],
            timestamp: now,
        }),
        ThreadEvent::ToolExecutionStarted(ToolExecutionStartedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "Tokyo"}),
            timestamp: now,
        }),
        ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            result: r#"{"temp": 18}"#.to_string(),
            timestamp: now,
        }),
        ThreadEvent::ToolExecutionFailed(ToolExecutionFailedData {
            tool_use_id: "toolu_456".to_string(),
            name: "broken_tool".to_string(),
            error: "Tool not found".to_string(),
            timestamp: now,
        }),
        ThreadEvent::AgentIterationCompleted(AgentIterationCompletedData {
            iteration: 1,
            has_tool_uses: true,
            timestamp: now,
        }),
        ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: 2,
            final_response: "The weather is 18°C".to_string(),
            timestamp: now,
        }),
        ThreadEvent::AgentFailed(AgentFailedData {
            error: "MaxIterationsReached".to_string(),
            details: "Exceeded 10 iterations".to_string(),
            iteration: 10,
            timestamp: now,
        }),
    ];

    store
        .append_events("thread-all-types", events.clone(), None, None)
        .await
        .expect("Should append all event types");

    let read_events = store
        .read_thread_events("thread-all-types")
        .await
        .expect("Should read events");

    assert_eq!(
        read_events.len(),
        events.len(),
        "Should have all events back"
    );

    // Verify each event type round-trips correctly
    for (i, (original, read)) in events.iter().zip(read_events.iter()).enumerate() {
        assert_eq!(
            std::mem::discriminant(original),
            std::mem::discriminant(read),
            "Event {} should have same variant",
            i
        );
    }
}

// =============================================================================
// Category Verification Test
// =============================================================================

#[tokio::test]
async fn test_thread_category() {
    setup_test!(_docker, _container, store);

    let now = Utc::now();

    // Write events to multiple different threads
    for i in 0..3 {
        let thread_id = format!("thread-category-{}", i);
        let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: format!("Message {}", i),
            timestamp: now,
        });

        store
            .append_event(&thread_id, event, None, None)
            .await
            .expect("Should append event");
    }

    // Verify each thread has its own stream
    for i in 0..3 {
        let thread_id = format!("thread-category-{}", i);
        let events = store
            .read_thread_events(&thread_id)
            .await
            .expect("Should read events");

        assert_eq!(events.len(), 1, "Each thread should have one event");

        match &events[0] {
            ThreadEvent::UserMessageReceived(data) => {
                assert_eq!(data.message, format!("Message {}", i));
            }
            _ => panic!("Wrong event type"),
        }
    }
}
