//! Provider trait for LLM implementations

use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;

use super::{error::LlmError, types::{GenerateRequest, Model, StreamEvent}};
use crate::llm::claude::ClaudeClient;
use crate::llm::gemini::GeminiClient;

/// Main interface that all LLM provider implementations must satisfy
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Stream generate content from the LLM
    ///
    /// This method sends a request to the LLM and returns a stream of events
    /// representing the incremental response.
    ///
    /// # Arguments
    /// * `request` - The generation request with messages, tools, and config
    ///
    /// # Returns
    /// A pinned boxed stream of `StreamEvent` results, or an error if the request fails
    async fn stream_generate(
        &self,
        request: GenerateRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, LlmError>;
}

/// Create an LLM provider from a model specification
///
/// This factory function creates the appropriate provider client based on the model.
/// Both Claude and Gemini clients connect to Google Cloud Vertex AI.
///
/// # Arguments
///
/// * `model` - The model to use (Claude or Gemini variant)
/// * `project_id` - GCP project ID
/// * `location` - GCP location/region (e.g., "us-central1")
///
/// # Returns
///
/// A boxed trait object implementing `LlmProvider`, or an error if client creation fails
///
/// # Example
///
/// ```rust,no_run
/// use rust2::llm::{Model, ClaudeModel, create_provider};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = create_provider(
///     Model::Claude(ClaudeModel::Sonnet45),
///     "my-project".to_string(),
///     "us-central1".to_string(),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_provider(
    model: Model,
    project_id: String,
    location: String,
) -> Result<Box<dyn LlmProvider>, LlmError> {
    match model {
        Model::Claude(claude_model) => {
            let client = ClaudeClient::new(project_id, location, claude_model).await?;
            Ok(Box::new(client))
        }
        Model::Gemini(gemini_model) => {
            let client = GeminiClient::new(project_id, location, gemini_model).await?;
            Ok(Box::new(client))
        }
    }
}
