use crate::llm::core::error::LlmError;

/// Errors that can occur during agent execution
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Error from the LLM provider
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    /// Failed to parse tool input JSON
    #[error("Failed to parse tool input: {0}")]
    ToolInputParse(#[from] serde_json::Error),

    /// LLM stream ended unexpectedly
    #[error("Stream ended unexpectedly")]
    UnexpectedStreamEnd,

    /// Maximum iterations reached without completion
    #[error("Maximum iterations reached ({0})")]
    MaxIterationsReached(usize),
}
