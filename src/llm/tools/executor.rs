//! Tool executor trait and implementations

use async_trait::async_trait;

/// Trait for executing tool calls from the LLM
///
/// Implementations of this trait handle the actual execution of tools requested by the LLM.
/// The trait accepts the tool use ID, function name, and arguments as a JSON value, and returns
/// either a success result (as a string) or an error message.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call
    ///
    /// # Arguments
    ///
    /// * `tool_use_id` - Unique identifier for this tool invocation
    /// * `name` - Name of the tool to execute
    /// * `arguments` - Tool arguments as a JSON value
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - Successful execution result (JSON string)
    /// * `Err(String)` - Error message describing what went wrong
    async fn execute(
        &self,
        tool_use_id: String,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<String, String>;
}
