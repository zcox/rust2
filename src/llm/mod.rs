//! LLM Abstraction Layer
//!
//! This module provides a unified interface for interacting with Anthropic Claude
//! and Google Gemini models hosted on Google Cloud Platform's Vertex AI.

pub mod agent;
pub mod auth;
pub mod claude;
pub mod core;
pub mod gemini;
pub mod http;
pub mod tools;

// Re-export commonly used types
pub use core::{
    config::GenerationConfig,
    error::LlmError,
    provider::{create_provider, LlmProvider},
    types::{
        ContentBlock, ContentDelta, FinishReason, GenerateRequest, Message, MessageRole, Model,
        StreamEvent, ToolDeclaration, UsageMetadata,
    },
};

pub use agent::{Agent, AgentError, AgentEvent, EventSourcedAgent, ThreadStore};
pub use claude::ClaudeModel;
pub use gemini::GeminiModel;
pub use tools::{create_tool_declaration, FunctionRegistry, ToolExecutor};
