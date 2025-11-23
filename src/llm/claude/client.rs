//! Claude client implementation

use async_trait::async_trait;
use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use std::pin::Pin;

use crate::llm::auth::adc::AuthenticationManager;
use crate::llm::core::{
    error::LlmError,
    provider::LlmProvider,
    types::{GenerateRequest, StreamEvent, UsageMetadata},
};

use super::mapper::{from_claude_event, to_claude_request};
use super::sse::parse_sse_stream;

/// Claude model identifiers for Vertex AI
#[derive(Debug, Clone)]
pub enum ClaudeModel {
    /// Claude Sonnet 4.5 (released 2025-09-29)
    Sonnet45,
    /// Claude Haiku 4.5 (released 2025-10-01)
    Haiku45,
}

impl ClaudeModel {
    /// Get the model identifier string for Vertex AI
    pub fn as_str(&self) -> &str {
        match self {
            ClaudeModel::Sonnet45 => "claude-sonnet-4-5@20250929",
            ClaudeModel::Haiku45 => "claude-haiku-4-5@20251001",
        }
    }
}

/// Client for interacting with Claude models on Vertex AI
pub struct ClaudeClient {
    /// HTTP client for making requests
    http_client: Client,
    /// Authentication manager for ADC tokens
    auth_manager: AuthenticationManager,
    /// GCP project ID
    project_id: String,
    /// GCP location (region)
    location: String,
    /// Model to use
    model: ClaudeModel,
}

impl ClaudeClient {
    /// Create a new Claude client
    ///
    /// # Arguments
    ///
    /// * `project_id` - GCP project ID
    /// * `location` - GCP location (e.g., "us-central1")
    /// * `model` - Claude model to use
    ///
    /// # Errors
    ///
    /// Returns an error if authentication initialization fails.
    pub async fn new(
        project_id: String,
        location: String,
        model: ClaudeModel,
    ) -> Result<Self, LlmError> {
        let http_client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| LlmError::HttpError {
                status: 0,
                body: format!("Failed to create HTTP client: {}", e),
            })?;

        let auth_manager = AuthenticationManager::new().await?;

        Ok(Self {
            http_client,
            auth_manager,
            project_id,
            location,
            model,
        })
    }

    /// Build the endpoint URL for streaming
    fn build_endpoint_url(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:streamRawPredict",
            self.location, self.project_id, self.location, self.model.as_str()
        )
    }

    /// Make a streaming request to Claude
    async fn make_streaming_request(
        &self,
        request: GenerateRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, LlmError> {
        // Convert to Claude request format
        let claude_request = to_claude_request(request);

        // Get auth token
        let token = self.auth_manager.get_token().await?;

        // Build request
        let url = self.build_endpoint_url();
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&claude_request)
            .send()
            .await?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| String::new());
            return Err(LlmError::HttpError {
                status: status.as_u16(),
                body,
            });
        }

        // Parse SSE stream
        let byte_stream = response.bytes_stream();
        let sse_stream = parse_sse_stream(Box::pin(byte_stream));

        // Convert to StreamEvent stream
        let mut accumulated_usage = UsageMetadata::new(0, 0);

        let event_stream = sse_stream.flat_map(move |result| {
            match result {
                Ok(claude_event) => {
                    // Convert Claude event to our abstraction events
                    let events = from_claude_event(claude_event, &mut accumulated_usage);
                    futures::stream::iter(
                        events
                            .into_iter()
                            .map(Ok)
                            .collect::<Vec<Result<StreamEvent, LlmError>>>(),
                    )
                }
                Err(e) => futures::stream::iter(vec![Err(e)]),
            }
        });

        Ok(Box::pin(event_stream))
    }
}

#[async_trait]
impl LlmProvider for ClaudeClient {
    async fn stream_generate(
        &self,
        request: GenerateRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, LlmError> {
        self.make_streaming_request(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_model_as_str() {
        assert_eq!(ClaudeModel::Sonnet45.as_str(), "claude-sonnet-4-5@20250929");
        assert_eq!(ClaudeModel::Haiku45.as_str(), "claude-haiku-4-5@20251001");
    }

    #[test]
    fn test_model_endpoint_url_format() {
        // Test URL construction logic without creating a full client
        let project_id = "my-project";
        let location = "us-central1";
        let model = ClaudeModel::Sonnet45;

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:streamRawPredict",
            location, project_id, location, model.as_str()
        );

        assert!(url.contains("us-central1-aiplatform.googleapis.com"));
        assert!(url.contains("my-project"));
        assert!(url.contains("claude-sonnet-4-5@20250929"));
        assert!(url.contains("publishers/anthropic"));
        assert!(url.contains("streamRawPredict"));
    }
}
