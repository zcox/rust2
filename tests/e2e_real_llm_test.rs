//! End-to-end tests with real LLM integration
//!
//! These tests use actual Claude API via Vertex AI and are marked with #[ignore]
//! to only run when explicitly requested with credentials configured.
//!
//! # Prerequisites
//!
//! - GCP project with Vertex AI API enabled
//! - Application Default Credentials configured: `gcloud auth application-default login`
//! - Environment variable: `GCP_PROJECT_ID`
//!
//! # Running
//!
//! ```bash
//! export GCP_PROJECT_ID=your-project-id
//! cargo test --test e2e_real_llm_test -- --ignored --nocapture
//! ```

mod common;

use async_trait::async_trait;
use dotenvy::dotenv;
use futures_util::StreamExt;
use reqwest::Client;
use rust2::llm::agent::{EventSourcedAgent, ThreadStore};
use rust2::llm::claude::client::{ClaudeClient, ClaudeModel};
use rust2::llm::core::config::GenerationConfig;
use rust2::llm::core::types::ToolDeclaration;
use rust2::llm::tools::executor::ToolExecutor;
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::routes::configure_routes;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::clients::Cli;
use uuid::Uuid;

/// Check if GCP credentials are available
fn check_credentials() -> Option<String> {
    // Load .env file if it exists
    let _ = dotenv();
    std::env::var("GCP_PROJECT_ID").ok()
}

/// Skip test if credentials not available
macro_rules! require_credentials {
    () => {
        match check_credentials() {
            Some(id) => id,
            None => {
                eprintln!("Skipping test: GCP_PROJECT_ID not set");
                eprintln!("Set GCP_PROJECT_ID and configure Application Default Credentials to run this test");
                return;
            }
        }
    };
}

/// Mock tool executor for testing
struct MockToolExecutor;

#[async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute(
        &self,
        _tool_use_id: String,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        // Simple calculator tool for testing
        if name == "calculator" {
            if let Some(expression) = arguments.get("expression").and_then(|v| v.as_str()) {
                // Very simple evaluation - just handle "X * Y" format
                if let Some((a, b)) = expression.split_once(" * ") {
                    if let (Ok(num_a), Ok(num_b)) =
                        (a.trim().parse::<i32>(), b.trim().parse::<i32>())
                    {
                        return Ok(json!({ "result": num_a * num_b }).to_string());
                    }
                }
            }
            return Err("Invalid expression format".to_string());
        }

        // Mock result for other tools
        Ok(json!({
            "tool": name,
            "arguments": arguments,
            "result": "Mock tool execution result"
        })
        .to_string())
    }
}

/// Start a test server with real LLM integration
async fn start_test_server_with_real_llm(
    project_id: String,
    tool_declarations: Vec<ToolDeclaration>,
) -> (
    String,
    Arc<ThreadStore>,
    testcontainers::Container<'static, testcontainers::GenericImage>,
) {
    // Start MessageDB container
    let docker = Box::leak(Box::new(Cli::default()));
    let container = docker.run(common::create_message_db_container());

    // Give the container time to initialize
    tokio::time::sleep(Duration::from_secs(3)).await;

    let host_port = container.get_host_port_ipv4(common::POSTGRES_PORT);
    let connection_string = common::build_connection_string("127.0.0.1", host_port);

    // Create MessageDB client and store
    let config = MessageDbConfig::from_connection_string(&connection_string).unwrap();
    let db_client = MessageDbClient::new(config).await.unwrap();
    let thread_store = Arc::new(ThreadStore::new(db_client));

    // Create REAL Claude client
    let location = std::env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());
    let claude_client = ClaudeClient::new(
        project_id,
        location,
        ClaudeModel::Haiku45, // Use Haiku for faster/cheaper tests
    )
    .await
    .expect("Failed to create Claude client");

    let llm_provider: Arc<dyn rust2::llm::core::provider::LlmProvider> = Arc::new(claude_client);

    // Create tool executor
    let tool_executor: Arc<dyn ToolExecutor> = Arc::new(MockToolExecutor);

    // Create EventSourcedAgent with real LLM
    let generation_config = GenerationConfig::new(1024);
    let system_prompt = Some(
        "You are a helpful AI assistant. Be concise and accurate in your responses.".to_string(),
    );

    let agent = Arc::new(EventSourcedAgent::new(
        llm_provider,
        tool_executor,
        (*thread_store).clone(),
        tool_declarations,
        generation_config,
        system_prompt,
    ));

    // Configure routes
    let routes = configure_routes(agent, thread_store.clone());

    // Start server on random port
    let port = portpicker::pick_unused_port().expect("No free ports");
    let server_url = format!("http://127.0.0.1:{}", port);

    // Spawn server in background
    tokio::spawn(async move {
        warp::serve(routes).run(([127, 0, 0, 1], port)).await;
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    (server_url, thread_store, container)
}

// =============================================================================
// Real LLM Integration Tests
// =============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --test e2e_real_llm_test -- --ignored
async fn test_real_llm_simple_conversation() {
    let project_id = require_credentials!();

    println!("Starting test with real Claude API...");

    // Start test server with real LLM
    let (server_url, thread_store, _container) =
        start_test_server_with_real_llm(project_id, vec![]).await;

    let client = Client::new();
    let thread_id = Uuid::new_v4();

    println!("Posting message to thread: {}", thread_id);

    // POST: "What is 2+2? Just answer with the number."
    let response = client
        .post(format!("{}/api/v1/threads/{}", server_url, thread_id))
        .json(&json!({
            "text": "What is 2+2? Just answer with the number."
        }))
        .send()
        .await
        .expect("Failed to send POST request");

    assert!(response.status().is_success(), "POST request failed");

    // Consume SSE stream
    let mut full_response = String::new();
    let mut done_received = false;
    let mut sse_buffer = String::new();

    let mut byte_stream = response.bytes_stream();
    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result.expect("Failed to read chunk");
        let text = String::from_utf8_lossy(&chunk);
        sse_buffer.push_str(&text);

        // Process complete SSE events (events are separated by double newlines)
        let mut current_event_type = String::new();
        for line in sse_buffer.lines() {
            if line.is_empty() {
                // Empty line marks end of event - reset
                current_event_type.clear();
                continue;
            }

            if line.starts_with("event:") {
                current_event_type = line.trim_start_matches("event:").trim().to_string();
                if current_event_type == "done" {
                    done_received = true;
                }
            } else if line.starts_with("data:") {
                let data = line.trim_start_matches("data:").trim();
                if !data.is_empty() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        // The field is "chunk" for agent_text events, not "text"
                        if let Some(text) = json.get("chunk").and_then(|v| v.as_str()) {
                            print!("{}", text);
                            full_response.push_str(text);
                        }
                    }
                }
            }
        }
    }
    println!(); // New line after response

    println!("LLM Response: {}", full_response);

    // Verify response
    assert!(done_received, "Should receive 'done' event");
    assert!(!full_response.is_empty(), "Should receive text from LLM");
    assert!(
        full_response.contains("4"),
        "Response should contain '4': {}",
        full_response
    );

    // GET thread to verify persistence
    println!("Getting thread history...");
    let get_response = client
        .get(format!("{}/api/v1/threads/{}", server_url, thread_id))
        .send()
        .await
        .expect("Failed to send GET request");

    assert!(get_response.status().is_success(), "GET request failed");

    let thread_data: serde_json::Value = get_response
        .json()
        .await
        .expect("Failed to parse GET response");

    println!(
        "Thread data: {}",
        serde_json::to_string_pretty(&thread_data).unwrap()
    );

    // Verify conversation stored correctly
    let messages = thread_data
        .get("messages")
        .and_then(|v| v.as_array())
        .expect("Should have messages array");

    assert_eq!(
        messages.len(),
        2,
        "Should have 2 messages (user + assistant)"
    );

    // Verify user message
    assert_eq!(
        messages[0].get("message_type").and_then(|v| v.as_str()),
        Some("user")
    );
    assert!(messages[0]
        .get("content")
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .unwrap()
        .contains("2+2"));

    // Verify assistant message
    assert_eq!(
        messages[1].get("message_type").and_then(|v| v.as_str()),
        Some("agent")
    );

    // Query ThreadStore directly to verify events
    let events = thread_store
        .read_thread_events(&thread_id.to_string())
        .await
        .expect("Failed to read thread events");

    println!("Event count: {}", events.len());
    assert!(
        events.len() > 0,
        "Should have persisted events in MessageDB"
    );

    // Verify key event types exist
    let event_types: Vec<String> = events.iter().map(|e| e.event_type().to_string()).collect();
    println!("Event types: {:?}", event_types);

    assert!(
        event_types.contains(&"UserMessageReceived".to_string()),
        "Should have UserMessageReceived event"
    );
    assert!(
        event_types.contains(&"AgentCompleted".to_string()),
        "Should have AgentCompleted event"
    );

    println!("✓ Real LLM simple conversation test passed!");
}

#[tokio::test]
#[ignore] // Run with: cargo test --test e2e_real_llm_test -- --ignored
async fn test_real_llm_with_tools() {
    let project_id = require_credentials!();

    println!("Starting test with real Claude API and tools...");

    // Define calculator tool
    let calculator_tool = ToolDeclaration {
        name: "calculator".to_string(),
        description: "Evaluates a mathematical expression. Use this to perform calculations."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate (e.g., '15 * 23')"
                }
            },
            "required": ["expression"]
        }),
    };

    // Start test server with tool
    let (server_url, thread_store, _container) =
        start_test_server_with_real_llm(project_id, vec![calculator_tool]).await;

    let client = Client::new();
    let thread_id = Uuid::new_v4();

    println!("Posting message with tool request to thread: {}", thread_id);

    // POST: Ask to use calculator
    let response = client
        .post(format!("{}/api/v1/threads/{}", server_url, thread_id))
        .json(&json!({
            "text": "What is 15 times 23? Use the calculator tool to compute this."
        }))
        .send()
        .await
        .expect("Failed to send POST request");

    assert!(response.status().is_success(), "POST request failed");

    // Consume SSE stream
    let mut tool_call_received = false;
    let mut tool_response_received = false;
    let mut done_received = false;
    let mut final_text = String::new();

    let mut byte_stream = response.bytes_stream();
    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result.expect("Failed to read chunk");
        let text = String::from_utf8_lossy(&chunk);

        // Parse SSE events
        let mut current_event_type = String::new();
        for line in text.lines() {
            if line.starts_with("event:") {
                current_event_type = line.trim_start_matches("event:").trim().to_string();
            } else if line.starts_with("data:") {
                let data = line.trim_start_matches("data:").trim();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    match current_event_type.as_str() {
                        "tool_call" => {
                            println!("Tool call event: {}", json);
                            tool_call_received = true;
                            assert_eq!(
                                json.get("tool_name").and_then(|v| v.as_str()),
                                Some("calculator"),
                                "Tool call should be for calculator"
                            );
                        }
                        "tool_response" => {
                            println!("Tool response event: {}", json);
                            tool_response_received = true;
                            // Verify result contains 345
                            let response_str = json.to_string();
                            assert!(
                                response_str.contains("345"),
                                "Tool response should contain result 345: {}",
                                response_str
                            );
                        }
                        "agent_text" => {
                            if let Some(chunk) = json.get("chunk").and_then(|v| v.as_str()) {
                                final_text.push_str(chunk);
                            }
                        }
                        "done" => {
                            done_received = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    println!("Final response text: {}", final_text);

    // Verify tool use flow
    assert!(tool_call_received, "Should receive tool_call event");
    assert!(tool_response_received, "Should receive tool_response event");
    assert!(done_received, "Should receive done event");
    assert!(
        final_text.contains("345"),
        "Final answer should mention 345: {}",
        final_text
    );

    // Verify events in MessageDB
    let events = thread_store
        .read_thread_events(&thread_id.to_string())
        .await
        .expect("Failed to read thread events");

    let event_types: Vec<String> = events.iter().map(|e| e.event_type().to_string()).collect();
    println!("Event types: {:?}", event_types);

    // Verify tool execution events
    assert!(
        event_types.contains(&"LlmToolUseStarted".to_string()),
        "Should have LlmToolUseStarted event"
    );
    assert!(
        event_types.contains(&"ToolExecutionStarted".to_string()),
        "Should have ToolExecutionStarted event"
    );
    assert!(
        event_types.contains(&"ToolExecutionCompleted".to_string()),
        "Should have ToolExecutionCompleted event"
    );
    assert!(
        event_types.contains(&"AgentCompleted".to_string()),
        "Should have AgentCompleted event"
    );

    println!("✓ Real LLM with tools test passed!");
}

#[tokio::test]
#[ignore] // Run with: cargo test --test e2e_real_llm_test -- --ignored
async fn test_real_llm_streaming_quality() {
    let project_id = require_credentials!();

    println!("Starting streaming quality test with real Claude API...");

    // Start test server with real LLM
    let (server_url, _thread_store, _container) =
        start_test_server_with_real_llm(project_id, vec![]).await;

    let client = Client::new();
    let thread_id = Uuid::new_v4();

    println!("Posting message to test streaming: {}", thread_id);

    // POST: Request a haiku (should stream over time)
    let response = client
        .post(format!("{}/api/v1/threads/{}", server_url, thread_id))
        .json(&json!({
            "text": "Write a haiku about coding. Take your time and be creative."
        }))
        .send()
        .await
        .expect("Failed to send POST request");

    assert!(response.status().is_success(), "POST request failed");

    // Track streaming timing
    let start_time = std::time::Instant::now();
    let mut first_event_time: Option<std::time::Instant> = None;
    let mut last_event_time: Option<std::time::Instant> = None;
    let mut event_count = 0;
    let mut full_response = String::new();

    let mut byte_stream = response.bytes_stream();
    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result.expect("Failed to read chunk");
        let text = String::from_utf8_lossy(&chunk);

        // Parse SSE events
        for line in text.lines() {
            if line.starts_with("data:") {
                let data = line.trim_start_matches("data:").trim();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(chunk) = json.get("chunk").and_then(|v| v.as_str()) {
                        if !chunk.is_empty() {
                            event_count += 1;
                            if first_event_time.is_none() {
                                first_event_time = Some(std::time::Instant::now());
                            }
                            last_event_time = Some(std::time::Instant::now());
                            full_response.push_str(chunk);
                        }
                    }
                }
            }
        }
    }

    let total_duration = start_time.elapsed();
    let streaming_duration = if let (Some(first), Some(last)) = (first_event_time, last_event_time)
    {
        last.duration_since(first)
    } else {
        Duration::from_secs(0)
    };

    println!("Full response:\n{}", full_response);
    println!("Event count: {}", event_count);
    println!("Total duration: {:?}", total_duration);
    println!("Streaming duration: {:?}", streaming_duration);

    // Verify streaming behavior
    assert!(
        event_count > 1,
        "Should receive multiple events (streaming), got {}",
        event_count
    );
    assert!(
        streaming_duration > Duration::from_millis(100),
        "Events should stream over time (not all at once), duration: {:?}",
        streaming_duration
    );
    assert!(
        !full_response.is_empty(),
        "Should receive a complete response"
    );

    // Haiku quality check (basic)
    let line_count = full_response
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    assert!(
        line_count >= 3,
        "Haiku should have at least 3 lines, got {}",
        line_count
    );

    println!("✓ Real LLM streaming quality test passed!");
}
