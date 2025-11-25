# Tool Declaration Automation Guide

This guide shows how to use the `#[tool]` macro to automatically generate tool declarations, eliminating manual JSON schema definitions.

## Quick Start

### 1. Define your argument struct with `JsonSchema`

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
struct CalculatorArgs {
    /// The operation to perform
    operation: Operation,
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}
```

**Key points:**
- Use doc comments (`///`) for field descriptions - they appear in the schema
- Derive both `Deserialize` (for serde) and `JsonSchema` (for schema generation)
- Use serde attributes like `#[serde(rename_all = "lowercase")]` to control serialization

### 2. Apply the `#[tool]` macro to your function

```rust
use rust2_tool_macros::tool;

#[tool(description = "Perform basic arithmetic operations (add, subtract, multiply, divide)")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Your implementation
    Ok(CalculatorResult { result: 42.0 })
}
```

**The macro automatically generates:**
- `CALCULATOR_TOOL_NAME` - A constant with the tool name
- `calculator_tool_declaration()` - A function that returns the `ToolDeclaration`

### 3. Register the tool

```rust
use rust2::llm::FunctionRegistry;

let mut registry = FunctionRegistry::new();
registry.register_async(CALCULATOR_TOOL_NAME, calculator);

let tool_declarations = vec![calculator_tool_declaration()];
```

## Before and After Comparison

### ❌ Before (Manual Schema - ~60 lines)

```rust
#[derive(Deserialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Manual registration
let mut registry = FunctionRegistry::new();
registry.register_async("calculator", calculator);

// Manual schema definition (30+ lines of JSON!)
let tool_declarations = vec![ToolDeclaration {
    name: "calculator".to_string(),
    description: "Perform basic arithmetic operations".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["add", "subtract", "multiply", "divide"],
                "description": "The operation to perform"
            },
            "a": {
                "type": "number",
                "description": "First operand"
            },
            "b": {
                "type": "number",
                "description": "Second operand"
            }
        },
        "required": ["operation", "a", "b"]
    }),
}];
```

### ✅ After (With Macro - ~15 lines)

```rust
#[derive(Deserialize, JsonSchema)]
struct CalculatorArgs {
    /// The operation to perform
    operation: Operation,
    /// First operand
    a: f64,
    /// Second operand
    b: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation { Add, Subtract, Multiply, Divide }

#[tool(description = "Perform basic arithmetic operations")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Simple registration
let mut registry = FunctionRegistry::new();
registry.register_async(CALCULATOR_TOOL_NAME, calculator);
let tool_declarations = vec![calculator_tool_declaration()];
```

## Benefits

1. **75% Less Boilerplate** - Reduced from ~60 lines to ~15 lines
2. **Type Safety** - Schema is always in sync with struct definition
3. **Single Source of Truth** - Doc comments serve as both code docs and schema descriptions
4. **Better Type Modeling** - Use enums instead of strings with validation
5. **Compile-Time Validation** - Schema errors caught at compile time, not runtime
6. **Easier Maintenance** - Change the struct once, schema updates automatically

## Advanced Features

### Custom Tool Name

Override the default tool name (which is the function name):

```rust
#[tool(name = "calc", description = "A calculator")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // ...
}
```

### Schema Constraints

Use `schemars` attributes for validation:

```rust
#[derive(Deserialize, JsonSchema)]
struct Args {
    /// Percentage value between 0 and 100
    #[schemars(range(min = 0, max = 100))]
    percentage: u8,

    /// Optional email address
    #[schemars(regex = r"^[^@]+@[^@]+\.[^@]+$")]
    email: Option<String>,
}
```

### Complex Types

The schema generation handles nested types, arrays, options, and more:

```rust
#[derive(Deserialize, JsonSchema)]
struct ComplexArgs {
    /// List of items to process
    items: Vec<String>,

    /// Optional configuration
    config: Option<Config>,

    /// Map of key-value pairs
    metadata: HashMap<String, String>,
}

#[derive(Deserialize, JsonSchema)]
struct Config {
    /// Enable verbose mode
    verbose: bool,
    /// Maximum retries
    max_retries: u32,
}
```

## Migration Guide

To migrate existing tools:

1. Add `schemars = "0.8"` to your dependencies
2. Add `JsonSchema` derive to your arg structs
3. Add doc comments to struct fields
4. Apply `#[tool]` macro to functions
5. Replace manual registration with generated constants/functions
6. Remove manual `ToolDeclaration` creation

See `examples/agent_calculator.rs` for a complete working example.
