//! Integration tests for Claude client
//!
//! These tests require valid GCP credentials and will make real API calls.
//! To run these tests:
//! 1. Copy `.env.example` to `.env` and fill in your GCP project ID
//! 2. Ensure you have valid credentials (run `gcloud auth application-default login`)
//! 3. Run: `cargo test --test claude_integration_test -- --ignored`

use futures::StreamExt;
use rust2::llm::{
    claude::{ClaudeClient, ClaudeModel},
    core::{
        config::GenerationConfig,
        provider::LlmProvider,
        types::{
            ContentBlock, ContentDelta, FinishReason, GenerateRequest, Message, MessageRole,
            StreamEvent, ToolDeclaration,
        },
    },
};
use std::env;

/// Helper to create a test client
async fn create_test_client() -> ClaudeClient {
    dotenvy::dotenv().ok();

    let project_id = env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required in .env");
    let location = env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    ClaudeClient::new(project_id, location, ClaudeModel::Haiku45)
        .await
        .expect("Failed to create Claude client")
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_simple_generation() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("What is 2+2? Answer with just the number.")],
        tools: None,
        config: GenerationConfig::new(100),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut text = String::new();
    let mut token_count = 0;

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text: t },
                ..
            } => {
                text.push_str(&t);
            }
            StreamEvent::MessageEnd { usage, .. } => {
                token_count = usage.total_tokens;
            }
            _ => {}
        }
    }

    println!("Response: {}", text);
    println!("Total tokens: {}", token_count);

    assert!(!text.is_empty());
    assert!(text.contains("4"));
    assert!(token_count > 0);
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_with_system_prompt() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("What should I do?")],
        tools: None,
        config: GenerationConfig::new(200),
        system: Some("You are a helpful pirate. Always respond like a pirate.".to_string()),
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut text = String::new();

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text: t },
                ..
            } => {
                text.push_str(&t);
            }
            _ => {}
        }
    }

    println!("Pirate response: {}", text);
    assert!(!text.is_empty());
    // The response should have some pirate-like characteristics
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_with_temperature() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("Say hello in a creative way")],
        tools: None,
        config: GenerationConfig::new(150).with_temperature(0.9),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut text = String::new();

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text: t },
                ..
            } => {
                text.push_str(&t);
            }
            _ => {}
        }
    }

    println!("Creative greeting: {}", text);
    assert!(!text.is_empty());
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_max_tokens() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("Write a very long essay about the ocean")],
        tools: None,
        config: GenerationConfig::new(50), // Very low limit
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut finish_reason = None;

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::MessageEnd {
                finish_reason: reason,
                ..
            } => {
                finish_reason = Some(reason);
            }
            _ => {}
        }
    }

    println!("Finish reason: {:?}", finish_reason);
    // Should finish due to max tokens
    assert!(finish_reason.is_some());
    assert_eq!(finish_reason.unwrap(), FinishReason::MaxTokens);
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_tool_call() {
    let client = create_test_client().await;

    let weather_tool = ToolDeclaration {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g. San Francisco, CA"
                }
            },
            "required": ["location"]
        }),
    };

    let request = GenerateRequest {
        messages: vec![Message::user("What's the weather in San Francisco?")],
        tools: Some(vec![weather_tool]),
        config: GenerationConfig::new(500),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut tool_use_id = None;
    let mut tool_name = None;
    let mut tool_input_json = String::new();
    let mut finish_reason = None;

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentBlockStart { block, .. } => {
                println!("Block start: {:?}", block);
                if let rust2::llm::core::types::ContentBlockStart::ToolUse { id, name } = block {
                    tool_use_id = Some(id);
                    tool_name = Some(name);
                }
            }
            StreamEvent::ContentDelta { delta, .. } => {
                if let ContentDelta::ToolUseDelta { partial } = delta {
                    tool_input_json.push_str(&partial.partial_json);
                    println!("Accumulated tool JSON: {}", tool_input_json);
                }
            }
            StreamEvent::MessageEnd {
                finish_reason: reason,
                ..
            } => {
                finish_reason = Some(reason);
            }
            _ => {}
        }
    }

    println!("Tool use ID: {:?}", tool_use_id);
    println!("Tool name: {:?}", tool_name);
    println!("Tool input: {}", tool_input_json);
    println!("Finish reason: {:?}", finish_reason);

    assert!(tool_use_id.is_some());
    assert_eq!(tool_name.as_deref(), Some("get_weather"));
    assert!(!tool_input_json.is_empty());
    assert_eq!(finish_reason, Some(FinishReason::ToolUse));

    // Parse the accumulated JSON to verify it's valid
    let parsed: serde_json::Value = serde_json::from_str(&tool_input_json)
        .expect("Tool input should be valid JSON");
    assert!(parsed.get("location").is_some());
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_tool_use_with_result() {
    let client = create_test_client().await;

    let weather_tool = ToolDeclaration {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g. San Francisco, CA"
                }
            },
            "required": ["location"]
        }),
    };

    // First request: model calls tool
    let request = GenerateRequest {
        messages: vec![Message::user("What's the weather in Tokyo?")],
        tools: Some(vec![weather_tool.clone()]),
        config: GenerationConfig::new(500),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut tool_use_id = None;
    let mut tool_input_json = String::new();

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentBlockStart { block, .. } => {
                if let rust2::llm::core::types::ContentBlockStart::ToolUse { id, .. } = block {
                    tool_use_id = Some(id);
                }
            }
            StreamEvent::ContentDelta { delta, .. } => {
                if let ContentDelta::ToolUseDelta { partial } = delta {
                    tool_input_json.push_str(&partial.partial_json);
                }
            }
            _ => {}
        }
    }

    let tool_id = tool_use_id.expect("Should have tool use ID");

    // Second request: provide tool result and continue
    let request2 = GenerateRequest {
        messages: vec![
            Message::user("What's the weather in Tokyo?"),
            Message {
                role: MessageRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: tool_id.clone(),
                    name: "get_weather".to_string(),
                    input: serde_json::from_str(&tool_input_json)
                        .expect("Valid JSON"),
                }],
            },
            Message::tool_result(tool_id, "The weather in Tokyo is sunny, 22°C"),
        ],
        tools: Some(vec![weather_tool]),
        config: GenerationConfig::new(500),
        system: None,
    };

    let mut stream2 = client
        .stream_generate(request2)
        .await
        .expect("Failed to start stream");

    let mut response_text = String::new();

    while let Some(event) = stream2.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text },
                ..
            } => {
                response_text.push_str(&text);
            }
            _ => {}
        }
    }

    println!("Final response: {}", response_text);
    assert!(!response_text.is_empty());
    // Should mention the weather information
    assert!(response_text.to_lowercase().contains("tokyo")
            || response_text.to_lowercase().contains("sunny")
            || response_text.to_lowercase().contains("22"));
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_parallel_tool_calls() {
    let client = create_test_client().await;

    let weather_tool = ToolDeclaration {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["location"]
        }),
    };

    let request = GenerateRequest {
        messages: vec![Message::user(
            "What's the weather in San Francisco and Tokyo?",
        )],
        tools: Some(vec![weather_tool]),
        config: GenerationConfig::new(1000),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut tool_calls = Vec::new();
    let mut current_tool: Option<(String, String, String)> = None; // (id, name, json)

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentBlockStart { block, .. } => {
                if let rust2::llm::core::types::ContentBlockStart::ToolUse { id, name } = block {
                    current_tool = Some((id, name, String::new()));
                }
            }
            StreamEvent::ContentDelta { delta, .. } => {
                if let ContentDelta::ToolUseDelta { partial } = delta {
                    if let Some((_, _, json)) = &mut current_tool {
                        json.push_str(&partial.partial_json);
                    }
                }
            }
            StreamEvent::ContentBlockEnd { .. } => {
                if let Some((id, name, json)) = current_tool.take() {
                    tool_calls.push((id, name, json));
                }
            }
            _ => {}
        }
    }

    println!("Tool calls: {:?}", tool_calls);

    // Claude may or may not use parallel tool calls for this prompt
    // Just verify that at least one tool call was made
    assert!(!tool_calls.is_empty());
    assert!(tool_calls.iter().all(|(_, name, _)| name == "get_weather"));
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_streaming_events() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("Count from 1 to 5")],
        tools: None,
        config: GenerationConfig::new(100),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut event_types = Vec::new();

    while let Some(event) = stream.next().await {
        let event = event.expect("Stream error");
        let event_name = format!("{:?}", event)
            .split('{')
            .next()
            .unwrap()
            .trim()
            .to_string();
        event_types.push(event_name);
    }

    println!("Event sequence: {:?}", event_types);

    // Should have MessageStart, ContentBlockStart, ContentDeltas, ContentBlockEnd, MessageEnd
    assert!(event_types.iter().any(|e| e.contains("MessageStart")));
    assert!(event_types.iter().any(|e| e.contains("ContentBlockStart")));
    assert!(event_types.iter().any(|e| e.contains("ContentDelta")));
    assert!(event_types.iter().any(|e| e.contains("MessageEnd")));
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_multi_turn_conversation() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![
            Message::user("My favorite color is purple."),
            Message::assistant("That's wonderful! Purple is a very regal color."),
            Message::user("What is my favorite color?"),
        ],
        tools: None,
        config: GenerationConfig::new(100),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut text = String::new();

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text: t },
                ..
            } => {
                text.push_str(&t);
            }
            _ => {}
        }
    }

    println!("Response: {}", text);
    assert!(!text.is_empty());
    // Should remember that the favorite color is purple
    assert!(text.to_lowercase().contains("purple"));
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_claude_sonnet_model() {
    // Test that we can also use Sonnet model
    dotenvy::dotenv().ok();

    let project_id = env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required in .env");
    let location = env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    let client = ClaudeClient::new(project_id, location, ClaudeModel::Sonnet45)
        .await
        .expect("Failed to create Claude Sonnet client");

    let request = GenerateRequest {
        messages: vec![Message::user("Say 'Hello from Sonnet!'")],
        tools: None,
        config: GenerationConfig::new(50),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut text = String::new();

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text: t },
                ..
            } => {
                text.push_str(&t);
            }
            _ => {}
        }
    }

    println!("Sonnet response: {}", text);
    assert!(!text.is_empty());
}
