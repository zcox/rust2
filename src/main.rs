use rust2::llm::agent::{EventSourcedAgent, ThreadStore};
use rust2::llm::core::{
    config::GenerationConfig,
    provider::{create_provider, LlmProvider},
    types::Model,
};
use rust2::llm::tools::{
    builtin::calculator::{calculate, CalculatorArgs},
    create_tool_declaration,
    executor::ToolExecutor,
    registry::FunctionRegistry,
};
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use rust2::routes::configure_routes;
use std::sync::Arc;

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

    // 3. Create LLM provider
    let project_id =
        std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID environment variable not set");
    let location = std::env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());
    let model_name =
        std::env::var("MODEL_NAME").unwrap_or_else(|_| "gemini-2.5-flash".to_string());

    println!(
        "Creating LLM provider (model: {}, project: {}, location: {})...",
        model_name, project_id, location
    );

    let model = Model::from_str(&model_name).unwrap_or_else(|e| {
        eprintln!("Error parsing model name: {}", e);
        std::process::exit(1);
    });

    let llm_provider: Arc<dyn LlmProvider> = Arc::from(
        create_provider(model, project_id, location)
            .await
            .expect("Failed to create LLM provider"),
    );

    println!("✓ Created LLM provider ({})", model_name);

    // 4. Create tool registry and register calculator tool
    let mut registry = FunctionRegistry::new();

    // Register calculator tool
    let calculator_declaration = create_tool_declaration::<CalculatorArgs>(
        "calculator",
        "Perform basic arithmetic operations: add, subtract, multiply, or divide two numbers",
    );
    registry
        .register_sync_tool(calculate, calculator_declaration)
        .expect("Failed to register calculator tool");

    println!("✓ Registered calculator tool");

    // Get all tool declarations for the agent
    let tool_declarations = registry.get_declarations();
    let tool_executor: Arc<dyn ToolExecutor> = Arc::new(registry);
    println!("✓ Created tool executor with {} tools", tool_declarations.len());

    // 5. Create EventSourcedAgent
    let generation_config = GenerationConfig::new(2048); // max_tokens
    let system_prompt = Some(
        "You are a helpful AI assistant. Be concise and accurate in your responses.".to_string(),
    );

    let agent = Arc::new(EventSourcedAgent::new(
        llm_provider,
        tool_executor,
        thread_store.clone(),
        tool_declarations,
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
