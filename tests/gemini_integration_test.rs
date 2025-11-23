//! Integration tests for Gemini client
//!
//! These tests require valid GCP credentials and will make real API calls.
//! To run these tests:
//! 1. Copy `.env.example` to `.env` and fill in your GCP project ID
//! 2. Ensure you have valid credentials (run `gcloud auth application-default login`)
//! 3. Run: `cargo test --test gemini_tests -- --ignored`

use futures::StreamExt;
use rust2::llm::{
    core::{
        config::GenerationConfig,
        provider::LlmProvider,
        types::{ContentDelta, GenerateRequest, Message, StreamEvent, ToolDeclaration},
    },
    gemini::{GeminiClient, GeminiModel},
};
use std::env;

/// Helper to create a test client
async fn create_test_client() -> GeminiClient {
    dotenvy::dotenv().ok();

    let project_id = env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required in .env");
    let location = env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    GeminiClient::new(project_id, location, GeminiModel::Gemini25Flash)
        .await
        .expect("Failed to create Gemini client")
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_gemini_simple_generation() {
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
async fn test_gemini_with_temperature() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("Say hello in a creative way")],
        tools: None,
        config: GenerationConfig::new(100).with_temperature(0.9),
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
async fn test_gemini_max_tokens() {
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
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_gemini_system_prompt() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![Message::user("What should I do?")],
        tools: None,
        config: GenerationConfig::new(100),
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
    // (though we can't guarantee exact words)
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_gemini_tool_call() {
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
        config: GenerationConfig::new(100),
        system: None,
    };

    let mut stream = client
        .stream_generate(request)
        .await
        .expect("Failed to start stream");

    let mut tool_calls = Vec::new();

    while let Some(event) = stream.next().await {
        match event.expect("Stream error") {
            StreamEvent::ContentBlockStart { block, .. } => {
                println!("Block start: {:?}", block);
            }
            StreamEvent::ContentDelta { delta, .. } => {
                println!("Delta: {:?}", delta);
                if let ContentDelta::ToolUseDelta { partial } = delta {
                    tool_calls.push(partial);
                }
            }
            StreamEvent::MessageEnd { finish_reason, .. } => {
                println!("Finish reason: {:?}", finish_reason);
            }
            _ => {}
        }
    }

    println!("Tool calls: {:?}", tool_calls);
    assert!(!tool_calls.is_empty());
    // Should have called get_weather
    assert!(tool_calls
        .iter()
        .any(|tc| tc.name.as_ref().map(|n| n == "get_weather").unwrap_or(false)));
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_gemini_streaming_events() {
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
        event_types.push(format!("{:?}", event).split('{').next().unwrap().trim().to_string());
    }

    println!("Event sequence: {:?}", event_types);

    // Should have at least MessageStart, some ContentDeltas, and MessageEnd
    assert!(event_types.iter().any(|e| e.contains("MessageStart")));
    assert!(event_types.iter().any(|e| e.contains("ContentDelta")));
    assert!(event_types.iter().any(|e| e.contains("MessageEnd")));
}

#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_gemini_multi_turn_conversation() {
    let client = create_test_client().await;

    let request = GenerateRequest {
        messages: vec![
            Message::user("My favorite color is blue."),
            Message::assistant("That's nice! Blue is a calming color."),
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
    // Should remember that the favorite color is blue
    assert!(text.to_lowercase().contains("blue"));
}
