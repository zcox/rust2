//! Provider trait for LLM implementations

use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;

use super::{error::LlmError, types::{GenerateRequest, StreamEvent}};

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
