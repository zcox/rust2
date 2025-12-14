//! Handler Integration Tests
//!
//! Integration tests for POST and GET handlers using real MessageDB
//! (testcontainers) with mock LLM provider.

mod common;

use async_trait::async_trait;
use chrono::Utc;
use rust2::llm::agent::{events::*, EventSourcedAgent, ThreadStore};
use rust2::llm::core::{
    config::GenerationConfig,
    error::LlmError,
    provider::LlmProvider,
    types::{
        ContentBlockStart, ContentDelta, FinishReason, GenerateRequest, MessageMetadata,
        MessageRole, PartialToolCall, StreamEvent, UsageMetadata,
    },
};
use rust2::llm::tools::executor::ToolExecutor;
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::models::{MessageType, SendMessageRequest, ThreadResponse};
use rust2::routes::configure_routes;
use serde_json::json;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use testcontainers::clients::Cli;
use uuid::Uuid;

// =============================================================================
// Mock LLM Provider
// =============================================================================

/// Mock LLM provider that returns predictable, pre-configured responses
struct MockLlmProvider {
    /// Pre-configured responses (one per LLM call)
    responses: Vec<Vec<StreamEvent>>,
    /// Current call index
    call_count: Arc<Mutex<usize>>,
}

impl MockLlmProvider {
    /// Create a new mock provider with simple text response
    // fn with_text_response(text: impl Into<String>) -> Self {
    //     let text = text.into();
    //     let events = vec![
    //         StreamEvent::MessageStart {
    //             message: MessageMetadata {
    //                 message_id: "msg_test".to_string(),
    //                 role: MessageRole::Assistant,
    //                 usage: None,
    //             },
    //         },
    //         StreamEvent::ContentBlockStart {
    //             index: 0,
    //             block: ContentBlockStart::Text { text: text.clone() },
    //         },
    //         StreamEvent::MessageEnd {
    //             finish_reason: FinishReason::EndTurn,
    //             usage: UsageMetadata {
    //                 input_tokens: 10,
    //                 output_tokens: 20,
    //                 total_tokens: 30,
    //             },
    //         },
    //     ];

    //     Self {
    //         responses: vec![events],
    //         call_count: Arc::new(Mutex::new(0)),
    //     }
    // }

    /// Create a new mock provider with multiple text responses
    fn with_multiple_text_responses(texts: Vec<impl Into<String> + Clone>) -> Self {
        let responses: Vec<Vec<StreamEvent>> = texts
            .into_iter()
            .map(|text| {
                let text = text.into();
                vec![
                    StreamEvent::MessageStart {
                        message: MessageMetadata {
                            message_id: "msg_test".to_string(),
                            role: MessageRole::Assistant,
                            usage: None,
                        },
                    },
                    StreamEvent::ContentBlockStart {
                        index: 0,
                        block: ContentBlockStart::Text { text: text.clone() },
                    },
                    StreamEvent::MessageEnd {
                        finish_reason: FinishReason::EndTurn,
                        usage: UsageMetadata {
                            input_tokens: 10,
                            output_tokens: 20,
                            total_tokens: 30,
                        },
                    },
                ]
            })
            .collect();

        Self {
            responses,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a provider that streams text in chunks
    #[allow(dead_code)]
    fn with_streaming_text(chunks: Vec<String>) -> Self {
        let mut events = vec![
            StreamEvent::MessageStart {
                message: MessageMetadata {
                    message_id: "msg_test".to_string(),
                    role: MessageRole::Assistant,
                    usage: None,
                },
            },
            StreamEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockStart::Text {
                    text: String::new(),
                },
            },
        ];

        for chunk in chunks {
            events.push(StreamEvent::ContentDelta {
                index: 0,
                delta: ContentDelta::TextDelta { text: chunk },
            });
        }

        events.push(StreamEvent::ContentBlockEnd { index: 0 });
        events.push(StreamEvent::MessageEnd {
            finish_reason: FinishReason::EndTurn,
            usage: UsageMetadata {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            },
        });

        Self {
            responses: vec![events],
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a provider that returns tool use
    fn with_tool_use(
        tool_name: impl Into<String>,
        tool_input: serde_json::Value,
        then_text: impl Into<String>,
    ) -> Self {
        let tool_name = tool_name.into();
        let tool_id = "toolu_test123".to_string();
        let input_json = serde_json::to_string(&tool_input).unwrap();

        // First response: tool use
        let first_response = vec![
            StreamEvent::MessageStart {
                message: MessageMetadata {
                    message_id: "msg_test1".to_string(),
                    role: MessageRole::Assistant,
                    usage: None,
                },
            },
            StreamEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockStart::ToolCall {
                    tool_call_id: tool_id.clone(),
                    name: tool_name.clone(),
                },
            },
            StreamEvent::ContentDelta {
                index: 0,
                delta: ContentDelta::ToolCallDelta {
                    partial: PartialToolCall {
                        tool_call_id: Some(tool_id.clone()),
                        name: Some(tool_name.clone()),
                        partial_json: input_json,
                    },
                },
            },
            StreamEvent::ContentBlockEnd { index: 0 },
            StreamEvent::MessageEnd {
                finish_reason: FinishReason::ToolUse,
                usage: UsageMetadata {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                },
            },
        ];

        // Second response: text after tool execution
        let text = then_text.into();
        let second_response = vec![
            StreamEvent::MessageStart {
                message: MessageMetadata {
                    message_id: "msg_test2".to_string(),
                    role: MessageRole::Assistant,
                    usage: None,
                },
            },
            StreamEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockStart::Text { text: text.clone() },
            },
            StreamEvent::MessageEnd {
                finish_reason: FinishReason::EndTurn,
                usage: UsageMetadata {
                    input_tokens: 15,
                    output_tokens: 10,
                    total_tokens: 25,
                },
            },
        ];

        Self {
            responses: vec![first_response, second_response],
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a provider that always returns tool use (infinite loop scenario)
    fn with_infinite_tool_use() -> Self {
        let events = vec![
            StreamEvent::MessageStart {
                message: MessageMetadata {
                    message_id: "msg_infinite".to_string(),
                    role: MessageRole::Assistant,
                    usage: None,
                },
            },
            StreamEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockStart::ToolCall {
                    tool_call_id: "toolu_infinite".to_string(),
                    name: "infinite_tool".to_string(),
                },
            },
            StreamEvent::ContentDelta {
                index: 0,
                delta: ContentDelta::ToolCallDelta {
                    partial: PartialToolCall {
                        tool_call_id: Some("toolu_infinite".to_string()),
                        name: Some("infinite_tool".to_string()),
                        partial_json: r#"{"test":"value"}"#.to_string(),
                    },
                },
            },
            StreamEvent::ContentBlockEnd { index: 0 },
            StreamEvent::MessageEnd {
                finish_reason: FinishReason::ToolUse,
                usage: UsageMetadata {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                },
            },
        ];

        // Return same tool-use response for many iterations
        Self {
            responses: vec![events.clone(); 20],
            call_count: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    async fn stream_generate(
        &self,
        _request: GenerateRequest,
    ) -> Result<
        Pin<Box<dyn futures::stream::Stream<Item = Result<StreamEvent, LlmError>> + Send>>,
        LlmError,
    > {
        let mut count = self.call_count.lock().unwrap();
        let index = *count;
        *count += 1;

        if index >= self.responses.len() {
            return Err(LlmError::StreamError(
                "Mock provider out of responses".to_string(),
            ));
        }

        let events = self.responses[index].clone();
        Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
    }

    fn provider_name(&self) -> &str {
        "mock"
    }

    fn model_id(&self) -> &str {
        "mock-model"
    }
}

// =============================================================================
// Mock Tool Executor
// =============================================================================

/// Mock tool executor that returns simple JSON results
struct MockToolExecutor;

#[async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute(
        &self,
        _tool_use_id: String,
        name: String,
        _arguments: serde_json::Value,
    ) -> Result<String, String> {
        match name.as_str() {
            "calculator" => Ok(json!({"result": 42}).to_string()),
            "get_weather" => Ok(json!({"temp": 72, "condition": "sunny"}).to_string()),
            "failing_tool" => Err("Tool execution failed".to_string()),
            _ => Ok(json!({"status": "ok"}).to_string()),
        }
    }
}

// =============================================================================
// Test Setup Helpers
// =============================================================================

/// Macro to setup test environment with MessageDB container
/// This keeps docker and container alive for the test duration
macro_rules! setup_test {
    ($docker:ident, $container:ident, $agent:ident, $store:ident) => {
        let $docker = Cli::default();
        let $container = $docker.run(common::create_message_db_container());

        // Give container time to fully initialize
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let host_port = $container.get_host_port_ipv4(common::POSTGRES_PORT);
        let connection_string = common::build_connection_string("127.0.0.1", host_port);
        let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
        let client = MessageDbClient::new(config).await.unwrap();
        let $store = Arc::new(ThreadStore::new(client.clone()));

        // Create mock provider with multiple responses (for tests that make multiple calls)
        let provider = Arc::new(MockLlmProvider::with_multiple_text_responses(
            vec!["Hello there!"; 10]
        ));
        let executor = Arc::new(MockToolExecutor);

        let $agent = Arc::new(EventSourcedAgent::new(
            provider,
            executor,
            ThreadStore::new(client),
            vec![],
            GenerationConfig::new(1024),
            None,
        ));
    };
}

/// Macro to setup test environment with custom mock provider
macro_rules! setup_test_with_provider {
    ($docker:ident, $container:ident, $agent:ident, $store:ident, $provider:expr) => {
        let $docker = Cli::default();
        let $container = $docker.run(common::create_message_db_container());

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let host_port = $container.get_host_port_ipv4(common::POSTGRES_PORT);
        let connection_string = common::build_connection_string("127.0.0.1", host_port);
        let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
        let client = MessageDbClient::new(config).await.unwrap();
        let $store = Arc::new(ThreadStore::new(client.clone()));

        let executor = Arc::new(MockToolExecutor);

        let $agent = Arc::new(EventSourcedAgent::new(
            $provider,
            executor,
            ThreadStore::new(client),
            vec![],
            GenerationConfig::new(1024),
            None,
        ));
    };
}

// =============================================================================
// POST Endpoint Tests
// =============================================================================

#[tokio::test]
async fn test_post_message_basic() {
    setup_test!(_docker, _container, agent, store);
    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id = Uuid::new_v4();
    let request_body = SendMessageRequest {
        text: "Hello".to_string(),
    };

    // POST to /api/v1/threads/{uuid}
    let response = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request_body)
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    // Parse SSE stream (simplified - just check we got response)
    let body = response.body();
    let body_str = std::str::from_utf8(body).unwrap();

    // Should contain agent_text and done events
    assert!(body_str.contains("event:agent_text") || body_str.contains("event:done"));

    // Verify events were persisted to MessageDB
    let events = store
        .read_thread_events(&thread_id.to_string())
        .await
        .unwrap();

    assert!(!events.is_empty(), "Events should be persisted");

    // Verify we have expected event types
    let has_user_message = events
        .iter()
        .any(|e| matches!(e, ThreadEvent::UserMessageReceived(_)));
    let has_agent_completed = events
        .iter()
        .any(|e| matches!(e, ThreadEvent::AgentCompleted(_)));

    assert!(has_user_message, "Should have UserMessageReceived event");
    assert!(has_agent_completed, "Should have AgentCompleted event");
}

#[tokio::test]
async fn test_post_message_sse_event_format() {
    setup_test!(_docker, _container, agent, store);
    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id = Uuid::new_v4();
    let request_body = SendMessageRequest {
        text: "Test message".to_string(),
    };

    let response = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request_body)
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let body = response.body();
    let body_str = std::str::from_utf8(body).unwrap();

    // Verify SSE format: should have "event:" and "data:" fields
    assert!(body_str.contains("event:"));
    assert!(body_str.contains("data:"));

    // Check for specific event types
    let has_done = body_str.contains("event:done");
    assert!(has_done, "Should have done event");

    // Verify data fields contain valid JSON
    for line in body_str.lines() {
        if line.starts_with("data:") {
            let json_str = line.trim_start_matches("data:").trim();
            if !json_str.is_empty() {
                let parsed: Result<serde_json::Value, _> = serde_json::from_str(json_str);
                assert!(
                    parsed.is_ok(),
                    "Data field should be valid JSON: {}",
                    json_str
                );
            }
        }
    }
}

#[tokio::test]
async fn test_post_message_with_tool_calls() {
    // Create provider that uses a tool
    let provider = Arc::new(MockLlmProvider::with_tool_use(
        "calculator",
        json!({"operation": "add", "a": 2, "b": 2}),
        "The answer is 42",
    ));

    setup_test_with_provider!(_docker, _container, agent, store, provider);
    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id = Uuid::new_v4();
    let request_body = SendMessageRequest {
        text: "What is 2+2?".to_string(),
    };

    let response = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request_body)
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let body = std::str::from_utf8(response.body()).unwrap();

    // Should contain tool_call and tool_response events
    assert!(
        body.contains("event:tool_call") || body.contains("tool_call"),
        "Should have tool_call event"
    );

    // Verify MessageDB contains tool execution events
    let events = store
        .read_thread_events(&thread_id.to_string())
        .await
        .unwrap();

    let has_tool_started = events
        .iter()
        .any(|e| matches!(e, ThreadEvent::ToolExecutionStarted(_)));
    let has_tool_completed = events
        .iter()
        .any(|e| matches!(e, ThreadEvent::ToolExecutionCompleted(_)));

    assert!(has_tool_started, "Should have ToolExecutionStarted event");
    assert!(
        has_tool_completed,
        "Should have ToolExecutionCompleted event"
    );
}

#[tokio::test]
async fn test_post_message_error_handling() {
    // TODO: This test would require creating an ErrorMockProvider
    // For now, we'll test max iterations scenario

    // Create provider that always returns tool use (infinite loop)
    let provider = Arc::new(MockLlmProvider::with_infinite_tool_use());

    // Setup manually with low max iterations
    let _docker = Cli::default();
    let _container = _docker.run(common::create_message_db_container());
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    let host_port = _container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string("127.0.0.1", host_port);
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let client = MessageDbClient::new(config).await.unwrap();
    let store = Arc::new(ThreadStore::new(client.clone()));
    let executor = Arc::new(MockToolExecutor);
    let agent = Arc::new(
        EventSourcedAgent::new(
            provider,
            executor,
            ThreadStore::new(client),
            vec![],
            GenerationConfig::new(1024),
            None,
        )
        .with_max_iterations(3),
    );

    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id = Uuid::new_v4();
    let request_body = SendMessageRequest {
        text: "Start infinite loop".to_string(),
    };

    let response = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request_body)
        .reply(&routes)
        .await;

    // Should still return 200 (SSE stream starts)
    assert_eq!(response.status(), 200);

    // Verify AgentFailed event was written
    let events = store
        .read_thread_events(&thread_id.to_string())
        .await
        .unwrap();

    let has_failed = events
        .iter()
        .any(|e| matches!(e, ThreadEvent::AgentFailed(_)));
    assert!(
        has_failed,
        "Should have AgentFailed event for max iterations"
    );
}

#[tokio::test]
async fn test_post_multiple_messages_same_thread() {
    setup_test!(_docker, _container, agent, store);
    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id = Uuid::new_v4();

    // POST first message
    let request1 = SendMessageRequest {
        text: "First message".to_string(),
    };
    let response1 = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request1)
        .reply(&routes)
        .await;

    assert_eq!(response1.status(), 200);

    // POST second message to same thread
    let request2 = SendMessageRequest {
        text: "Second message".to_string(),
    };
    let response2 = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request2)
        .reply(&routes)
        .await;

    assert_eq!(response2.status(), 200);

    // Verify both messages are in MessageDB
    let events = store
        .read_thread_events(&thread_id.to_string())
        .await
        .unwrap();

    let user_messages: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ThreadEvent::UserMessageReceived(data) => Some(data.message.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(user_messages.len(), 2);
    assert_eq!(user_messages[0], "First message");
    assert_eq!(user_messages[1], "Second message");
}

#[tokio::test]
async fn test_post_message_persistence_verification() {
    setup_test!(_docker, _container, agent, store);
    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id = Uuid::new_v4();
    let request_body = SendMessageRequest {
        text: "Test persistence".to_string(),
    };

    let response = warp::test::request()
        .method("POST")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .json(&request_body)
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    // Read events from MessageDB
    let events = store
        .read_thread_events(&thread_id.to_string())
        .await
        .unwrap();

    // Verify event order and types
    assert!(!events.is_empty());

    // First event should be UserMessageReceived
    match &events[0] {
        ThreadEvent::UserMessageReceived(data) => {
            assert_eq!(data.message, "Test persistence");
        }
        _ => panic!("First event should be UserMessageReceived"),
    }

    // Should have AgentIterationStarted
    assert!(events
        .iter()
        .any(|e| matches!(e, ThreadEvent::AgentIterationStarted(_))));

    // Should have LlmCallStarted
    assert!(events
        .iter()
        .any(|e| matches!(e, ThreadEvent::LlmCallStarted(_))));

    // Should have AgentCompleted as last meaningful event
    assert!(events
        .iter()
        .any(|e| matches!(e, ThreadEvent::AgentCompleted(_))));
}

#[tokio::test]
async fn test_post_concurrent_different_threads() {
    setup_test!(_docker, _container, agent, store);
    let routes = configure_routes(agent.clone(), store.clone());

    let thread_id_1 = Uuid::new_v4();
    let thread_id_2 = Uuid::new_v4();
    let thread_id_3 = Uuid::new_v4();

    // Spawn 3 concurrent requests
    let routes_1 = routes.clone();
    let routes_2 = routes.clone();
    let routes_3 = routes.clone();

    let handle_1 = tokio::spawn(async move {
        warp::test::request()
            .method("POST")
            .path(&format!("/api/v1/threads/{}", thread_id_1))
            .json(&SendMessageRequest {
                text: "Thread 1".to_string(),
            })
            .reply(&routes_1)
            .await
    });

    let handle_2 = tokio::spawn(async move {
        warp::test::request()
            .method("POST")
            .path(&format!("/api/v1/threads/{}", thread_id_2))
            .json(&SendMessageRequest {
                text: "Thread 2".to_string(),
            })
            .reply(&routes_2)
            .await
    });

    let handle_3 = tokio::spawn(async move {
        warp::test::request()
            .method("POST")
            .path(&format!("/api/v1/threads/{}", thread_id_3))
            .json(&SendMessageRequest {
                text: "Thread 3".to_string(),
            })
            .reply(&routes_3)
            .await
    });

    // Wait for all to complete
    let resp_1 = handle_1.await.unwrap();
    let resp_2 = handle_2.await.unwrap();
    let resp_3 = handle_3.await.unwrap();

    assert_eq!(resp_1.status(), 200);
    assert_eq!(resp_2.status(), 200);
    assert_eq!(resp_3.status(), 200);

    // Verify each thread has only its own events
    let events_1 = store
        .read_thread_events(&thread_id_1.to_string())
        .await
        .unwrap();
    let events_2 = store
        .read_thread_events(&thread_id_2.to_string())
        .await
        .unwrap();
    let events_3 = store
        .read_thread_events(&thread_id_3.to_string())
        .await
        .unwrap();

    // Each should have events
    assert!(!events_1.is_empty());
    assert!(!events_2.is_empty());
    assert!(!events_3.is_empty());

    // Verify correct messages
    let msg_1 = match &events_1[0] {
        ThreadEvent::UserMessageReceived(data) => data.message.clone(),
        _ => panic!("Expected UserMessageReceived"),
    };
    assert_eq!(msg_1, "Thread 1");
}

// =============================================================================
// GET Endpoint Tests
// =============================================================================

#[tokio::test]
async fn test_get_empty_thread() {
    setup_test!(_docker, _container, agent, store);
    let routes = configure_routes(agent, store);

    let thread_id = Uuid::new_v4();

    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();
    assert_eq!(thread_response.thread_id, thread_id);
    assert_eq!(thread_response.messages.len(), 0);
}

#[tokio::test]
async fn test_get_thread_after_single_message() {
    setup_test!(_docker, _container, agent, store);

    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    // Write events manually
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
        ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
            stop_reason: "end_turn".to_string(),
            content_blocks: vec![ContentBlockData::Text {
                text: "Hi there!".to_string(),
            }],
            timestamp: now,
        }),
        ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: 1,
            final_response: "Hi there!".to_string(),
            timestamp: now,
        }),
    ];

    store
        .append_events(&thread_id.to_string(), events, None, None)
        .await
        .unwrap();

    // GET thread
    let routes = configure_routes(agent, store);
    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();
    assert_eq!(thread_response.messages.len(), 2);

    // First message should be user
    assert_eq!(thread_response.messages[0].message_type, MessageType::User);

    // Second message should be agent
    assert_eq!(thread_response.messages[1].message_type, MessageType::Agent);
}

#[tokio::test]
async fn test_get_thread_with_tool_use() {
    setup_test!(_docker, _container, agent, store);

    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    // Write events for conversation with tool use
    let events = vec![
        ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "What's the weather?".to_string(),
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
        ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            result: json!({"temp": 72}).to_string(),
            timestamp: now,
        }),
        ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: 2,
            final_response: "It's 72°F".to_string(),
            timestamp: now,
        }),
    ];

    store
        .append_events(&thread_id.to_string(), events, None, None)
        .await
        .unwrap();

    // GET thread
    let routes = configure_routes(agent, store);
    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();

    // Should have messages: user, assistant, tool result
    assert!(thread_response.messages.len() >= 2);

    // Verify user message
    assert_eq!(thread_response.messages[0].message_type, MessageType::User);
}

#[tokio::test]
async fn test_get_thread_multi_turn_conversation() {
    setup_test!(_docker, _container, agent, store);

    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    // Create 3-turn conversation
    let mut events = vec![];

    // Turn 1
    events.push(ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "First question".to_string(),
        timestamp: now,
    }));
    events.push(ThreadEvent::LlmResponseCompleted(
        LlmResponseCompletedData {
            stop_reason: "end_turn".to_string(),
            content_blocks: vec![ContentBlockData::Text {
                text: "First answer".to_string(),
            }],
            timestamp: now,
        },
    ));

    // Turn 2
    events.push(ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "Second question".to_string(),
        timestamp: now,
    }));
    events.push(ThreadEvent::LlmResponseCompleted(
        LlmResponseCompletedData {
            stop_reason: "end_turn".to_string(),
            content_blocks: vec![ContentBlockData::Text {
                text: "Second answer".to_string(),
            }],
            timestamp: now,
        },
    ));

    // Turn 3
    events.push(ThreadEvent::UserMessageReceived(UserMessageReceivedData {
        message: "Third question".to_string(),
        timestamp: now,
    }));
    events.push(ThreadEvent::LlmResponseCompleted(
        LlmResponseCompletedData {
            stop_reason: "end_turn".to_string(),
            content_blocks: vec![ContentBlockData::Text {
                text: "Third answer".to_string(),
            }],
            timestamp: now,
        },
    ));

    store
        .append_events(&thread_id.to_string(), events, None, None)
        .await
        .unwrap();

    // GET thread
    let routes = configure_routes(agent, store);
    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();

    // Should have 6 messages (3 user + 3 assistant)
    assert_eq!(thread_response.messages.len(), 6);

    // Verify alternating pattern
    assert_eq!(thread_response.messages[0].message_type, MessageType::User);
    assert_eq!(thread_response.messages[1].message_type, MessageType::Agent);
    assert_eq!(thread_response.messages[2].message_type, MessageType::User);
    assert_eq!(thread_response.messages[3].message_type, MessageType::Agent);
    assert_eq!(thread_response.messages[4].message_type, MessageType::User);
    assert_eq!(thread_response.messages[5].message_type, MessageType::Agent);
}

#[tokio::test]
async fn test_get_thread_with_failed_iteration() {
    setup_test!(_docker, _container, agent, store);

    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    // Write events including failure
    let events = vec![
        ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Trigger failure".to_string(),
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
        ThreadEvent::AgentFailed(AgentFailedData {
            error: "MaxIterationsReached".to_string(),
            details: "Exceeded limit".to_string(),
            iteration: 10,
            timestamp: now,
        }),
    ];

    store
        .append_events(&thread_id.to_string(), events, None, None)
        .await
        .unwrap();

    // GET thread - should not crash
    let routes = configure_routes(agent, store);
    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();

    // Should at least have user message
    assert!(!thread_response.messages.is_empty());
    assert_eq!(thread_response.messages[0].message_type, MessageType::User);
}

#[tokio::test]
async fn test_get_thread_message_ordering() {
    setup_test!(_docker, _container, agent, store);

    let thread_id = Uuid::new_v4();
    let base_time = Utc::now();

    // Create many events in specific order
    let mut events = vec![];
    for i in 0..10 {
        let timestamp = base_time + chrono::Duration::seconds(i);

        events.push(ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: format!("Message {}", i),
            timestamp,
        }));

        events.push(ThreadEvent::LlmResponseCompleted(
            LlmResponseCompletedData {
                stop_reason: "end_turn".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: format!("Response {}", i),
                }],
                timestamp,
            },
        ));
    }

    store
        .append_events(&thread_id.to_string(), events, None, None)
        .await
        .unwrap();

    // GET thread
    let routes = configure_routes(agent, store);
    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    assert_eq!(response.status(), 200);

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();

    // Verify messages are in chronological order
    assert_eq!(thread_response.messages.len(), 20);

    // Verify timestamps increase monotonically
    for i in 1..thread_response.messages.len() {
        assert!(thread_response.messages[i].timestamp >= thread_response.messages[i - 1].timestamp);
    }
}

#[tokio::test]
async fn test_get_thread_performance_long_thread() {
    setup_test!(_docker, _container, agent, store);

    let thread_id = Uuid::new_v4();
    let now = Utc::now();

    // Create 100 events (10 turn conversation)
    let mut events = vec![];
    for i in 0..10 {
        events.push(ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: format!("Question {}", i),
            timestamp: now,
        }));

        events.push(ThreadEvent::AgentIterationStarted(
            AgentIterationStartedData {
                iteration: i + 1,
                timestamp: now,
            },
        ));

        events.push(ThreadEvent::LlmCallStarted(LlmCallStartedData {
            provider: "claude".to_string(),
            model: "sonnet".to_string(),
            message_count: i * 2 + 1,
            timestamp: now,
        }));

        events.push(ThreadEvent::LlmResponseCompleted(
            LlmResponseCompletedData {
                stop_reason: "end_turn".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: format!("Answer {}", i),
                }],
                timestamp: now,
            },
        ));

        events.push(ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: i + 1,
            final_response: format!("Answer {}", i),
            timestamp: now,
        }));
    }

    store
        .append_events(&thread_id.to_string(), events, None, None)
        .await
        .unwrap();

    // Measure GET time
    let start = std::time::Instant::now();

    let routes = configure_routes(agent, store);
    let response = warp::test::request()
        .method("GET")
        .path(&format!("/api/v1/threads/{}", thread_id))
        .reply(&routes)
        .await;

    let elapsed = start.elapsed();

    assert_eq!(response.status(), 200);

    // Should complete in reasonable time (< 1 second)
    assert!(
        elapsed.as_secs() < 1,
        "GET should complete quickly even with many events"
    );

    let thread_response: ThreadResponse = serde_json::from_slice(response.body()).unwrap();

    // Should have all messages
    assert_eq!(thread_response.messages.len(), 20); // 10 user + 10 assistant
}
