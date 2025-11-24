//! Example: Simple agent loop without tools
//!
//! This example demonstrates the basic agent loop with just text responses,
//! no tool calling. It shows how to stream agent responses and handle events.
//!
//! # Prerequisites
//!
//! 1. Set up Google Cloud Application Default Credentials:
//!    ```bash
//!    gcloud auth application-default login
//!    ```
//!
//! 2. Create a `.env` file in the project root with:
//!    ```
//!    GCP_PROJECT_ID=your-project-id
//!    GCP_LOCATION=us-central1
//!    ```
//!
//! # Running
//!
//! ```bash
//! cargo run --example agent_simple
//! ```

use futures::StreamExt;
use rust2::llm::{
    create_provider, Agent, AgentEvent, ClaudeModel, ContentDelta, GenerationConfig, Model,
    StreamEvent,
};
use std::env;
use std::io::Write;

// Empty tool executor since we're not using tools
struct NoOpExecutor;

#[async_trait::async_trait]
impl rust2::llm::ToolExecutor for NoOpExecutor {
    async fn execute(
        &self,
        _tool_use_id: String,
        _name: String,
        _arguments: serde_json::Value,
    ) -> Result<String, String> {
        Err("No tools available".to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get configuration from environment
    let project_id = env::var("GCP_PROJECT_ID").unwrap_or_else(|_| {
        eprintln!("Warning: GCP_PROJECT_ID not set in .env file, using placeholder");
        "your-project-id".to_string()
    });

    let location = env::var("GCP_LOCATION").unwrap_or_else(|_| {
        eprintln!("Warning: GCP_LOCATION not set in .env file, using us-central1");
        "us-central1".to_string()
    });

    println!("=== Simple Agent Example ===\n");
    println!("Configuration:");
    println!("  Project ID: {}", project_id);
    println!("  Location: {}", location);
    println!("  Model: Claude Haiku 4.5\n");

    println!("Creating LLM provider...");

    // Set up LLM provider (using Claude Haiku for speed)
    let provider = create_provider(
        Model::Claude(ClaudeModel::Haiku45),
        project_id,
        location,
    )
    .await?;

    println!("Creating agent...");

    // Create agent with no tools
    let mut agent = Agent::new(
        provider,
        Box::new(NoOpExecutor),
        vec![], // No tools
        GenerationConfig::new(1024).with_temperature(0.7),
        Some("You are a helpful assistant that provides concise, informative responses.".to_string()),
    );

    println!("Agent created. Starting conversation...\n");
    println!("----------------------------------------\n");

    // First turn
    {
        println!("User: What are the three laws of thermodynamics?\n");
        println!("Assistant: ");

        let mut stream = agent
            .run("What are the three laws of thermodynamics?")
            .await?;

        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::IterationStarted { iteration } => {
                    if iteration > 1 {
                        println!("[Iteration {}]", iteration);
                    }
                }
                AgentEvent::LlmEvent(StreamEvent::ContentDelta {
                    delta: ContentDelta::TextDelta { text },
                    ..
                }) => {
                    print!("{}", text);
                    std::io::stdout().flush()?;
                }
                AgentEvent::Completed => {
                    println!("\n");
                }
                _ => {}
            }
        }
    }

    // Second turn - follow-up question
    {
        println!("User: Can you explain the first law in simpler terms?\n");
        println!("Assistant: ");

        let mut stream = agent
            .run("Can you explain the first law in simpler terms?")
            .await?;

        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::IterationStarted { iteration } => {
                    if iteration > 1 {
                        println!("[Iteration {}]", iteration);
                    }
                }
                AgentEvent::LlmEvent(StreamEvent::ContentDelta {
                    delta: ContentDelta::TextDelta { text },
                    ..
                }) => {
                    print!("{}", text);
                    std::io::stdout().flush()?;
                }
                AgentEvent::Completed => {
                    println!("\n");
                }
                _ => {}
            }
        }
    }

    // Show conversation history
    println!("----------------------------------------");
    println!("\nConversation summary:");
    println!("Total messages: {}", agent.messages().len());
    println!("\nMessage breakdown:");
    for (i, msg) in agent.messages().iter().enumerate() {
        println!("  {}. {:?}", i + 1, msg.role);
    }

    Ok(())
}
