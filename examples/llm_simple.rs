/// Simple LLM Example
///
/// This example demonstrates the basic usage of the LLM abstraction layer:
/// - Creating a provider using create_provider
/// - Sending a simple prompt
/// - Streaming and printing the response
///
/// To run this example:
/// 1. Set up GCP Application Default Credentials:
///    gcloud auth application-default login
/// 2. Create a .env file in the project root with:
///    GCP_PROJECT_ID=your-project-id
///    GCP_LOCATION=us-central1
/// 3. Run: cargo run --example llm_simple

use futures::StreamExt;
use rust2::llm::{
    create_provider, ClaudeModel, GenerateRequest, GenerationConfig, Message, Model, StreamEvent,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== LLM Abstraction Layer - Simple Example ===\n");

    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get GCP configuration from environment
    let project_id = env::var("GCP_PROJECT_ID").unwrap_or_else(|_| {
        eprintln!("Warning: GCP_PROJECT_ID not set in .env file, using placeholder");
        "your-project-id".to_string()
    });

    let location = env::var("GCP_LOCATION").unwrap_or_else(|_| {
        eprintln!("Warning: GCP_LOCATION not set in .env file, using us-central1");
        "us-central1".to_string()
    });

    println!("Configuration:");
    println!("  Project ID: {}", project_id);
    println!("  Location: {}", location);
    println!("  Model: Claude Sonnet 4.5\n");

    // Create the LLM provider
    println!("Creating provider...");
    let provider = create_provider(
        Model::Claude(ClaudeModel::Sonnet45),
        project_id,
        location,
    )
    .await?;
    println!("✓ Provider created successfully\n");

    // Create a simple request
    let request = GenerateRequest {
        messages: vec![Message::user(
            "Write a haiku about Rust programming language.",
        )],
        tools: None,
        config: GenerationConfig::new(1024).with_temperature(0.7),
        system: Some("You are a helpful assistant that writes creative poetry.".to_string()),
    };

    println!("Sending request to LLM...");
    println!("Prompt: Write a haiku about Rust programming language.\n");

    // Stream the response
    let mut stream = provider.stream_generate(request).await?;

    println!("Response:");
    println!("─────────");

    let mut full_text = String::new();

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::MessageStart { message } => {
                println!("[Message started - ID: {}]", message.id);
            }
            StreamEvent::ContentBlockStart { index, block } => {
                println!("[Content block {} started]", index);
                if let rust2::llm::core::types::ContentBlockStart::Text { text } = block {
                    if !text.is_empty() {
                        print!("{}", text);
                        full_text.push_str(&text);
                    }
                }
            }
            StreamEvent::ContentDelta { delta, .. } => {
                if let rust2::llm::core::types::ContentDelta::TextDelta { text } = delta {
                    print!("{}", text);
                    full_text.push_str(&text);
                }
            }
            StreamEvent::ContentBlockEnd { index } => {
                println!("\n[Content block {} ended]", index);
            }
            StreamEvent::MessageEnd {
                finish_reason,
                usage,
            } => {
                println!("\n─────────");
                println!("✓ Message complete");
                println!("  Finish reason: {:?}", finish_reason);
                println!("  Tokens used:");
                println!("    Input: {}", usage.input_tokens);
                println!("    Output: {}", usage.output_tokens);
                println!("    Total: {}", usage.total_tokens);
            }
            StreamEvent::Error { error } => {
                eprintln!("\n✗ Error during streaming: {}", error);
            }
            _ => {
                // Handle other event types (MessageDelta, etc.)
            }
        }
    }

    println!("\n\n=== Full Response ===");
    println!("{}", full_text);

    println!("\n=== Example Complete ===");
    Ok(())
}
