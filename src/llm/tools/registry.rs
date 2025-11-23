//! Function registry for tool execution

use std::collections::HashMap;
use std::future::Future;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::executor::ToolExecutor;

/// Type alias for boxed async functions
type AsyncToolFn = Box<
    dyn Fn(serde_json::Value) -> BoxFuture<'static, Result<String, String>> + Send + Sync,
>;

/// Registry for managing tool functions
///
/// The `FunctionRegistry` allows you to register Rust functions that can be called by the LLM.
/// It handles automatic deserialization of arguments from JSON and serialization of results back to JSON.
///
/// # Example
///
/// ```ignore
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize)]
/// struct WeatherArgs {
///     location: String,
/// }
///
/// #[derive(Serialize)]
/// struct WeatherResult {
///     temperature: f32,
///     conditions: String,
/// }
///
/// async fn get_weather(args: WeatherArgs) -> Result<WeatherResult, String> {
///     // Implementation
///     Ok(WeatherResult {
///         temperature: 72.0,
///         conditions: "Sunny".to_string(),
///     })
/// }
///
/// let mut registry = FunctionRegistry::new();
/// registry.register_async("get_weather", get_weather);
/// ```
pub struct FunctionRegistry {
    functions: HashMap<String, AsyncToolFn>,
}

impl FunctionRegistry {
    /// Create a new empty function registry
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Register an async function that returns a serializable result
    ///
    /// # Type Parameters
    ///
    /// * `F` - The function type
    /// * `Args` - The argument type (must implement `DeserializeOwned`)
    /// * `R` - The result type (must implement `Serialize`)
    /// * `Fut` - The future type returned by the function
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool (must match what's declared to the LLM)
    /// * `func` - The async function to execute
    pub fn register_async<F, Args, R, Fut>(&mut self, name: impl Into<String>, func: F)
    where
        F: Fn(Args) -> Fut + Send + Sync + 'static,
        Args: DeserializeOwned + Send + 'static,
        R: Serialize + Send + 'static,
        Fut: Future<Output = Result<R, String>> + Send + 'static,
    {
        let wrapper = move |args_json: serde_json::Value| {
            // Deserialize arguments
            let args = match serde_json::from_value::<Args>(args_json) {
                Ok(args) => args,
                Err(e) => {
                    let err_msg = format!("Failed to deserialize arguments: {}", e);
                    return Box::pin(async move { Err(err_msg) }) as BoxFuture<'static, _>;
                }
            };

            // Call the function
            let future = func(args);

            // Box the future and handle serialization
            Box::pin(async move {
                match future.await {
                    Ok(result) => {
                        // Serialize the result
                        serde_json::to_string(&result)
                            .map_err(|e| format!("Failed to serialize result: {}", e))
                    }
                    Err(e) => Err(e),
                }
            }) as BoxFuture<'static, _>
        };

        self.functions.insert(name.into(), Box::new(wrapper));
    }

    /// Register a synchronous function that returns a serializable result
    ///
    /// # Type Parameters
    ///
    /// * `F` - The function type
    /// * `Args` - The argument type (must implement `DeserializeOwned`)
    /// * `R` - The result type (must implement `Serialize`)
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool (must match what's declared to the LLM)
    /// * `func` - The synchronous function to execute
    pub fn register_sync<F, Args, R>(&mut self, name: impl Into<String>, func: F)
    where
        F: Fn(Args) -> Result<R, String> + Send + Sync + 'static,
        Args: DeserializeOwned + Send + 'static,
        R: Serialize + Send + 'static,
    {
        let wrapper = move |args_json: serde_json::Value| {
            // Deserialize arguments
            let args = match serde_json::from_value::<Args>(args_json) {
                Ok(args) => args,
                Err(e) => {
                    let err_msg = format!("Failed to deserialize arguments: {}", e);
                    return Box::pin(async move { Err(err_msg) }) as BoxFuture<'static, _>;
                }
            };

            // Call the function
            let result = func(args);

            // Box the result as a future
            Box::pin(async move {
                match result {
                    Ok(result) => {
                        // Serialize the result
                        serde_json::to_string(&result)
                            .map_err(|e| format!("Failed to serialize result: {}", e))
                    }
                    Err(e) => Err(e),
                }
            }) as BoxFuture<'static, _>
        };

        self.functions.insert(name.into(), Box::new(wrapper));
    }

    /// Check if a function is registered
    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Get the number of registered functions
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Execute a registered function by name
    ///
    /// This is an internal method used by the `ToolExecutor` implementation.
    async fn execute_function(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        match self.functions.get(name) {
            Some(func) => func(arguments).await,
            None => Err(format!("Unknown tool: {}", name)),
        }
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolExecutor for FunctionRegistry {
    async fn execute(
        &self,
        _tool_use_id: String,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        self.execute_function(&name, arguments).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, PartialEq)]
    struct AddArgs {
        a: i32,
        b: i32,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct AddResult {
        sum: i32,
    }

    #[tokio::test]
    async fn test_register_sync_function() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("add", |args: AddArgs| {
            Ok(AddResult { sum: args.a + args.b })
        });

        assert!(registry.contains("add"));
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_sync_function() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("add", |args: AddArgs| {
            Ok(AddResult { sum: args.a + args.b })
        });

        let args = serde_json::json!({"a": 5, "b": 3});
        let result = registry.execute_function("add", args).await.unwrap();

        let parsed: AddResult = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, AddResult { sum: 8 });
    }

    #[tokio::test]
    async fn test_execute_async_function() {
        let mut registry = FunctionRegistry::new();

        registry.register_async("add_async", |args: AddArgs| async move {
            // Simulate async work
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            Ok(AddResult { sum: args.a + args.b })
        });

        let args = serde_json::json!({"a": 10, "b": 20});
        let result = registry.execute_function("add_async", args).await.unwrap();

        let parsed: AddResult = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, AddResult { sum: 30 });
    }

    #[tokio::test]
    async fn test_function_error() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("divide", |args: AddArgs| {
            if args.b == 0 {
                Err("Division by zero".to_string())
            } else {
                Ok(AddResult { sum: args.a / args.b })
            }
        });

        let args = serde_json::json!({"a": 10, "b": 0});
        let result = registry.execute_function("divide", args).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Division by zero");
    }

    #[tokio::test]
    async fn test_deserialization_error() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("add", |args: AddArgs| {
            Ok(AddResult { sum: args.a + args.b })
        });

        // Invalid arguments (missing field)
        let args = serde_json::json!({"a": 5});
        let result = registry.execute_function("add", args).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to deserialize arguments"));
    }

    #[tokio::test]
    async fn test_unknown_function() {
        let registry = FunctionRegistry::new();

        let args = serde_json::json!({"a": 5, "b": 3});
        let result = registry.execute_function("unknown", args).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unknown tool: unknown");
    }

    #[tokio::test]
    async fn test_tool_executor_trait() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("add", |args: AddArgs| {
            Ok(AddResult { sum: args.a + args.b })
        });

        let executor: &dyn ToolExecutor = &registry;
        let args = serde_json::json!({"a": 7, "b": 3});
        let result = executor.execute("tool-1".to_string(), "add".to_string(), args)
            .await
            .unwrap();

        let parsed: AddResult = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, AddResult { sum: 10 });
    }

    #[tokio::test]
    async fn test_multiple_functions() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("add", |args: AddArgs| {
            Ok(AddResult { sum: args.a + args.b })
        });

        registry.register_sync("multiply", |args: AddArgs| {
            Ok(AddResult { sum: args.a * args.b })
        });

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("add"));
        assert!(registry.contains("multiply"));

        let args = serde_json::json!({"a": 3, "b": 4});
        let result = registry.execute_function("multiply", args).await.unwrap();

        let parsed: AddResult = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed, AddResult { sum: 12 });
    }

    #[derive(Debug, Serialize)]
    struct ComplexResult {
        message: String,
        data: Vec<i32>,
        nested: NestedData,
    }

    #[derive(Debug, Serialize)]
    struct NestedData {
        value: String,
    }

    #[derive(Debug, Deserialize)]
    struct EmptyArgs {}

    #[tokio::test]
    async fn test_complex_serialization() {
        let mut registry = FunctionRegistry::new();

        registry.register_sync("get_data", |_args: EmptyArgs| {
            Ok(ComplexResult {
                message: "Success".to_string(),
                data: vec![1, 2, 3],
                nested: NestedData {
                    value: "nested".to_string(),
                },
            })
        });

        let args = serde_json::json!({});
        let result = registry.execute_function("get_data", args).await.unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["message"], "Success");
        assert_eq!(parsed["data"][0], 1);
        assert_eq!(parsed["nested"]["value"], "nested");
    }
}
