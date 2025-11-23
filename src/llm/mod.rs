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

// Re-export commonly used types
pub use core::{
    config::GenerationConfig,
    error::LlmError,
    provider::LlmProvider,
    types::{
        ContentBlock, ContentDelta, FinishReason, GenerateRequest, Message, MessageRole,
        StreamEvent, ToolDeclaration, UsageMetadata,
    },
};

pub use tools::{FunctionRegistry, ToolExecutor};
