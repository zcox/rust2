//! Example: Simple agent loop with calculator tool
//!
//! This example demonstrates how to use the agent to have a conversation
//! that involves calling tools. The agent will automatically detect when
//! the LLM wants to call a tool, execute it, and continue the conversation.
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
//! cargo run --example agent_calculator
//! ```

use futures::StreamExt;
use rust2::llm::{
    create_provider, Agent, AgentEvent, ClaudeModel, ContentDelta,
    GenerationConfig, Model, StreamEvent, FunctionRegistry,
};
use rust2_tool_macros::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Deserialize, JsonSchema)]
struct CalculatorArgs {
    /// The operation to perform
    operation: Operation,
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
}

#[derive(Serialize)]
struct CalculatorResult {
    result: f64,
}

/// Calculator tool function
///
/// The #[tool] macro automatically generates a module `calculator_tool` containing:
/// - `NAME`: A constant with the tool name
/// - `declaration()`: Function returning the ToolDeclaration
/// - `execute`: Re-export of the original calculator function
///
/// This eliminates the need for manual JSON schema definitions!
#[tool(description = "Perform basic arithmetic operations (add, subtract, multiply, divide)")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    let result = match args.operation {
        Operation::Add => args.a + args.b,
        Operation::Subtract => args.a - args.b,
        Operation::Multiply => args.a * args.b,
        Operation::Divide => {
            if args.b == 0.0 {
                return Err("Division by zero".to_string());
            }
            args.a / args.b
        }
    };

    Ok(CalculatorResult { result })
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

    println!("=== Agent Calculator Example ===\n");
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

    println!("Setting up tools...");

    // Set up tools - now much simpler with the #[tool] macro module pattern!
    let mut registry = FunctionRegistry::new();

    // Register tools - declarations are stored internally in the registry
    registry.register(calculator_tool::registration())?;

    // For multiple tools, you could use the register_tools! macro:
    // register_tools!(registry, calculator_tool, weather_tool)?;

    // Get all registered tool declarations
    let tool_declarations = registry.get_declarations();

    // Create agent
    let mut agent = Agent::new(
        provider,
        Box::new(registry),
        tool_declarations,
        GenerationConfig::new(1024).with_temperature(0.7),
        Some("You are a helpful assistant with access to a calculator.".to_string()),
    );

    println!("Agent created. Starting conversation...\n");
    println!("----------------------------------------\n");

    // First turn - use a scope to ensure stream is dropped
    {
        println!("User: What is 15 multiplied by 23?\n");

        let mut stream = agent.run("What is 15 multiplied by 23?").await?;

        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::IterationStarted { iteration } => {
                    println!("[Iteration {}]", iteration);
                }
                AgentEvent::LlmEvent(StreamEvent::ContentDelta {
                    delta: ContentDelta::TextDelta { text },
                    ..
                }) => {
                    print!("{}", text);
                    std::io::stdout().flush()?;
                }
                AgentEvent::ToolExecutionStarted { name, input, .. } => {
                    println!("\n[Calling tool: {} with args: {}]", name, input);
                }
                AgentEvent::ToolExecutionCompleted { name, result, .. } => {
                    println!("[Tool {} completed: {}]", name, result);
                }
                AgentEvent::ToolExecutionFailed { name, error, .. } => {
                    println!("[Tool {} failed: {}]", name, error);
                }
                AgentEvent::Completed => {
                    println!("\n[Agent completed]\n");
                }
                _ => {}
            }
        }
    } // stream is dropped here

    // Second turn
    {
        println!("User: Now divide that by 5\n");

        let mut stream = agent.run("Now divide that by 5").await?;

        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::IterationStarted { iteration } => {
                    println!("[Iteration {}]", iteration);
                }
                AgentEvent::LlmEvent(StreamEvent::ContentDelta {
                    delta: ContentDelta::TextDelta { text },
                    ..
                }) => {
                    print!("{}", text);
                    std::io::stdout().flush()?;
                }
                AgentEvent::ToolExecutionStarted { name, input, .. } => {
                    println!("\n[Calling tool: {} with args: {}]", name, input);
                }
                AgentEvent::ToolExecutionCompleted { name, result, .. } => {
                    println!("[Tool {} completed: {}]", name, result);
                }
                AgentEvent::Completed => {
                    println!("\n[Agent completed]\n");
                }
                _ => {}
            }
        }
    } // stream is dropped here

    // Show conversation history
    println!("----------------------------------------");
    println!("\nFull conversation history:");
    for (i, msg) in agent.messages().iter().enumerate() {
        println!("{}. {:?} ({} content blocks)", i + 1, msg.role, msg.content.len());
    }

    Ok(())
}
