//! Tool execution framework
//!
//! This module provides the infrastructure for executing tool calls from LLMs.
//! It includes the `ToolExecutor` trait and the `FunctionRegistry` for managing
//! and executing registered tool functions.

pub mod declaration;
pub mod executor;
pub mod registry;

// Re-export commonly used types
pub use declaration::create_tool_declaration;
pub use executor::ToolExecutor;
pub use registry::{FunctionRegistry, RegistryError, ToolRegistration};

/// Helper macro to register multiple tools at once
///
/// This macro simplifies registering multiple tools that use the `#[tool]` macro
/// module pattern. It takes a registry and a list of tool module paths.
/// The declarations are stored internally in the registry.
///
/// # Example
///
/// ```ignore
/// #[tool(description = "Perform basic arithmetic operations")]
/// async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
///     // Implementation
/// }
///
/// #[tool(description = "Get the current weather")]
/// async fn weather(args: WeatherArgs) -> Result<WeatherResult, String> {
///     // Implementation
/// }
///
/// let mut registry = FunctionRegistry::new();
/// register_tools!(registry, calculator_tool, weather_tool);
///
/// let declarations = registry.get_declarations();
/// let agent = Agent::new(provider, Box::new(registry), declarations, config, prompt);
/// ```
#[macro_export]
macro_rules! register_tools {
    ($registry:expr, $($tool_mod:path),+ $(,)?) => {
        $(
            {
                use $tool_mod as tool;
                $registry.register_async_tool(tool::NAME, tool::execute, tool::declaration())?;
            }
        )+
    };
}
