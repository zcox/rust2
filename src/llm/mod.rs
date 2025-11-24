//! LLM Abstraction Layer
//!
//! This module provides a unified interface for interacting with Anthropic Claude
//! and Google Gemini models hosted on Google Cloud Platform's Vertex AI.

pub mod core;
pub mod auth;
pub mod gemini;
pub mod claude;
pub mod tools;
pub mod http;
pub mod agent;

// Re-export commonly used types
pub use core::{
    config::GenerationConfig,
    error::LlmError,
    provider::{create_provider, LlmProvider},
    types::{
        ContentBlock, ContentDelta, FinishReason, GenerateRequest, Message, MessageRole,
        Model, StreamEvent, ToolDeclaration, UsageMetadata,
    },
};

pub use claude::ClaudeModel;
pub use gemini::GeminiModel;
pub use tools::{FunctionRegistry, ToolExecutor};
pub use agent::{Agent, AgentError, AgentEvent};
