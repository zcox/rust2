//! Claude provider implementation
//!
//! This module provides a client for interacting with Anthropic Claude models
//! hosted on Google Cloud Platform's Vertex AI.

pub mod client;
pub mod mapper;
pub mod sse;
pub mod types;

// Re-export commonly used types
pub use client::{ClaudeClient, ClaudeModel};
