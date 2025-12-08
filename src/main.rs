use rust2::llm::agent::{EventSourcedAgent, ThreadStore};
use rust2::llm::core::{config::GenerationConfig, provider::LlmProvider};
use rust2::llm::gemini::{GeminiClient, GeminiModel};
use rust2::llm::tools::executor::ToolExecutor;
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::routes::configure_routes;
use std::sync::Arc;

use async_trait::async_trait;

/// Mock tool executor for development
/// TODO: Replace with real tool executor
struct MockToolExecutor;

#[async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute(
        &self,
        _tool_use_id: String,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        // Return mock result
        Ok(serde_json::json!({
            "tool": name,
            "arguments": arguments,
            "result": "Mock tool execution result"
        })
        .to_string())
    }
}

#[tokio::main]
async fn main() {
    println!("Initializing event-sourced agent API...");

    // Load environment variables from .env file
    dotenvy::dotenv().ok();
    println!("✓ Loaded .env file");

    // 1. Create MessageDB client
    println!("Connecting to MessageDB...");
    let db_config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:message_store_password@localhost:5433/message_store",
    )
    .expect("Failed to create MessageDB config");

    let db_client = MessageDbClient::new(db_config)
        .await
        .expect("Failed to connect to MessageDB");
    println!("✓ Connected to MessageDB");

    // 2. Create ThreadStore
    let thread_store = ThreadStore::new(db_client);
    println!("✓ Created ThreadStore");

    // 3. Create LLM provider (Gemini 2.5 Flash)
    let project_id =
        std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID environment variable not set");
    let location = std::env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    println!(
        "Creating Gemini client (project: {}, location: {})...",
        project_id, location
    );
    let gemini_client = GeminiClient::new(project_id, location, GeminiModel::Gemini25Flash)
        .await
        .expect("Failed to create Gemini client");

    let llm_provider: Arc<dyn LlmProvider> = Arc::new(gemini_client);
    println!("✓ Created LLM provider (Gemini 2.5 Flash)");

    // 4. Create tool executor (mock for now)
    let tool_executor: Arc<dyn ToolExecutor> = Arc::new(MockToolExecutor);
    println!("✓ Created tool executor (mock)");

    // 5. Create EventSourcedAgent
    let generation_config = GenerationConfig::new(2048); // max_tokens
    let system_prompt = Some(
        "You are a helpful AI assistant. Be concise and accurate in your responses.".to_string(),
    );

    let agent = Arc::new(EventSourcedAgent::new(
        llm_provider,
        tool_executor,
        thread_store.clone(),
        vec![], // No tools for now
        generation_config,
        system_prompt,
    ));
    println!("✓ Created EventSourcedAgent");

    // 6. Configure routes with dependencies
    let routes = configure_routes(agent, Arc::new(thread_store));

    println!("\n🚀 Server ready!");
    println!("   POST http://127.0.0.1:3030/api/v1/threads/{{threadId}}");
    println!("   GET  http://127.0.0.1:3030/api/v1/threads/{{threadId}}");
    println!("\nListening on http://127.0.0.1:3030");

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
