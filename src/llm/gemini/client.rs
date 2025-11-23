//! Gemini client implementation

use async_trait::async_trait;
use futures::stream::Stream;
use futures::StreamExt;
use reqwest::Client;
use std::pin::Pin;
use uuid::Uuid;

use crate::llm::auth::adc::AuthenticationManager;
use crate::llm::core::{
    error::LlmError,
    provider::LlmProvider,
    types::{GenerateRequest, StreamEvent},
};

use super::mapper::{create_message_start, from_gemini_response, to_gemini_request};
use super::sse::parse_sse_stream;

/// Gemini model identifiers
#[derive(Debug, Clone)]
pub enum GeminiModel {
    /// Gemini 2.5 Pro
    Gemini25Pro,
    /// Gemini 2.5 Flash
    Gemini25Flash,
    /// Gemini 2.5 Flash Lite
    Gemini25FlashLite,
}

impl GeminiModel {
    /// Get the model identifier string
    pub fn as_str(&self) -> &str {
        match self {
            GeminiModel::Gemini25Pro => "gemini-2.5-pro",
            GeminiModel::Gemini25Flash => "gemini-2.5-flash",
            GeminiModel::Gemini25FlashLite => "gemini-2.5-flash-lite",
        }
    }
}

/// Client for interacting with Gemini models on Vertex AI
pub struct GeminiClient {
    /// HTTP client for making requests
    http_client: Client,
    /// Authentication manager for ADC tokens
    auth_manager: AuthenticationManager,
    /// GCP project ID
    project_id: String,
    /// GCP location (region)
    location: String,
    /// Model to use
    model: GeminiModel,
}

impl GeminiClient {
    /// Create a new Gemini client
    ///
    /// # Arguments
    ///
    /// * `project_id` - GCP project ID
    /// * `location` - GCP location (e.g., "us-central1")
    /// * `model` - Gemini model to use
    ///
    /// # Errors
    ///
    /// Returns an error if authentication initialization fails.
    pub async fn new(
        project_id: String,
        location: String,
        model: GeminiModel,
    ) -> Result<Self, LlmError> {
        let http_client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| {
                LlmError::HttpError {
                    status: 0,
                    body: format!("Failed to create HTTP client: {}", e),
                }
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
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
            self.location, self.project_id, self.location, self.model.as_str()
        )
    }

    /// Make a streaming request to Gemini
    async fn make_streaming_request(
        &self,
        request: GenerateRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, LlmError> {
        // Convert to Gemini request format
        let gemini_request = to_gemini_request(request);

        // Get auth token
        let token = self.auth_manager.get_token().await?;

        // Build request
        let url = self.build_endpoint_url();
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&gemini_request)
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
        let message_id = Uuid::new_v4().to_string();
        let mut emitted_start = false;
        let mut current_index = 0;

        let event_stream = sse_stream.map(move |result| match result {
            Ok(gemini_response) => {
                let mut events = Vec::new();

                // Emit message start on first chunk
                if !emitted_start {
                    events.push(create_message_start(message_id.clone()));
                    emitted_start = true;
                }

                // Convert Gemini response to our events
                let mut response_events =
                    from_gemini_response(gemini_response, &mut current_index);
                events.append(&mut response_events);

                Ok(events)
            }
            Err(e) => Err(e),
        });

        // Flatten the stream of event vectors into individual events
        let flattened = event_stream.flat_map(|result| {
            futures::stream::iter(match result {
                Ok(events) => events.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
        });

        Ok(Box::pin(flattened))
    }
}

#[async_trait]
impl LlmProvider for GeminiClient {
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
    fn test_gemini_model_as_str() {
        assert_eq!(GeminiModel::Gemini25Pro.as_str(), "gemini-2.5-pro");
        assert_eq!(GeminiModel::Gemini25Flash.as_str(), "gemini-2.5-flash");
        assert_eq!(
            GeminiModel::Gemini25FlashLite.as_str(),
            "gemini-2.5-flash-lite"
        );
    }

    #[test]
    fn test_model_endpoint_url_format() {
        // Test URL construction logic without creating a full client
        let project_id = "my-project";
        let location = "us-central1";
        let model = GeminiModel::Gemini25Flash;

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
            location, project_id, location, model.as_str()
        );

        assert!(url.contains("us-central1-aiplatform.googleapis.com"));
        assert!(url.contains("my-project"));
        assert!(url.contains("gemini-2.5-flash"));
        assert!(url.contains("streamGenerateContent"));
        assert!(url.contains("alt=sse"));
    }
}
