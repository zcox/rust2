//! Tool execution framework
//!
//! This module provides the infrastructure for executing tool calls from LLMs.
//! It includes the `ToolExecutor` trait and the `FunctionRegistry` for managing
//! and executing registered tool functions.

pub mod executor;
pub mod registry;

// Re-export commonly used types
pub use executor::ToolExecutor;
pub use registry::FunctionRegistry;
