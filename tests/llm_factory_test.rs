//! Test for the LLM provider factory
//!
//! This test demonstrates using the factory pattern to create providers
//! from the unified Model enum.

use rust2::llm::{create_provider, ClaudeModel, GeminiModel, Model};

#[test]
fn test_model_enum_variants() {
    // Test that we can create Model enum variants
    let claude_model = Model::Claude(ClaudeModel::Sonnet45);
    let gemini_model = Model::Gemini(GeminiModel::Gemini25Flash);

    // Test as_str method
    assert_eq!(claude_model.as_str(), "claude-sonnet-4-5@20250929");
    assert_eq!(gemini_model.as_str(), "gemini-2.5-flash");
}

#[tokio::test]
#[ignore] // Run with --ignored flag since it requires GCP credentials
async fn test_create_provider_claude() {
    dotenvy::dotenv().ok();

    let project_id = std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required");
    let location = std::env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    let provider = create_provider(
        Model::Claude(ClaudeModel::Haiku45),
        project_id,
        location,
    )
    .await
    .expect("Failed to create Claude provider");

    // Just verify we got a provider back (it implements LlmProvider)
    // Actual functionality is tested in integration tests
    assert!(std::any::type_name_of_val(&*provider).contains("ClaudeClient"));
}

#[tokio::test]
#[ignore] // Run with --ignored flag since it requires GCP credentials
async fn test_create_provider_gemini() {
    dotenvy::dotenv().ok();

    let project_id = std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required");
    let location = std::env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    let provider = create_provider(
        Model::Gemini(GeminiModel::Gemini25Flash),
        project_id,
        location,
    )
    .await
    .expect("Failed to create Gemini provider");

    // Just verify we got a provider back
    assert!(std::any::type_name_of_val(&*provider).contains("GeminiClient"));
}

#[test]
fn test_all_claude_models() {
    // Verify all Claude model variants can be wrapped in Model enum
    let models = vec![
        Model::Claude(ClaudeModel::Sonnet45),
        Model::Claude(ClaudeModel::Haiku45),
    ];

    assert_eq!(models[0].as_str(), "claude-sonnet-4-5@20250929");
    assert_eq!(models[1].as_str(), "claude-haiku-4-5@20251001");
}

#[test]
fn test_all_gemini_models() {
    // Verify all Gemini model variants can be wrapped in Model enum
    let models = vec![
        Model::Gemini(GeminiModel::Gemini25Pro),
        Model::Gemini(GeminiModel::Gemini25Flash),
        Model::Gemini(GeminiModel::Gemini25FlashLite),
    ];

    assert_eq!(models[0].as_str(), "gemini-2.5-pro");
    assert_eq!(models[1].as_str(), "gemini-2.5-flash");
    assert_eq!(models[2].as_str(), "gemini-2.5-flash-lite");
}
