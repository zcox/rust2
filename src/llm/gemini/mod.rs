//! Gemini provider implementation
//!
//! This module provides a client for interacting with Google's Gemini models
//! via Vertex AI, implementing the LlmProvider trait.

pub mod client;
pub mod mapper;
pub mod sse;
pub mod types;

// Re-export main types for convenience
pub use client::{GeminiClient, GeminiModel};
