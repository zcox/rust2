# Tool Declaration Automation Plan

## Overview

This plan outlines the design for automating the creation of `ToolDeclaration` instances from Rust functions, eliminating the need to manually write JSON schemas for tool parameters. The goal is to use Rust's type system and procedural macros to derive tool metadata directly from function signatures.

## Current State

In the `agent_calculator.rs` example, developers must:

1. Define argument and result structs:
```rust
#[derive(Deserialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

#[derive(Serialize)]
struct CalculatorResult {
    result: f64,
}
```

2. Implement the tool function:
```rust
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}
```

3. Register the function:
```rust
registry.register_async("calculator", calculator);
```

4. **Manually create the tool declaration** (this is what we want to automate):
```rust
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

## Problems with Current Approach

1. **Duplication**: Schema mirrors the struct definition
2. **Error-prone**: Schema and struct can get out of sync
3. **Maintenance burden**: Changes to args require updating both struct and schema
4. **No compile-time validation**: Schema errors only discovered at runtime
5. **Verbose**: Large schemas become unwieldy

## Goal

Enable developers to write:

```rust
// Args struct uses doc comments and JsonSchema for parameter schema
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

// Tool metadata lives on the function (name defaults to function name)
#[tool(description = "Perform basic arithmetic operations")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}
```

And automatically get:
- JSON schema generation from the struct
- Tool declaration with proper metadata
- Registration in the function registry

## Proposed Architecture

### Phase 1: JSON Schema Generation

Use the `schemars` crate to automatically derive JSON schemas from Rust types.

**Dependencies:**
```toml
schemars = "0.8"
```

**Implementation:**

```rust
use schemars::{JsonSchema, schema_for};

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

// Generate schema at runtime
let schema = schema_for!(CalculatorArgs);
let input_schema = serde_json::to_value(&schema)?;
```

**Benefits:**
- Uses standard doc comments for descriptions
- Handles nested types, enums, options automatically
- Respects serde attributes (`rename`, `rename_all`, `skip`, etc.)
- Generates proper required fields list

### Phase 2: Tool Attribute Macro

Create a procedural macro `#[tool]` that is applied to functions to generate `ToolDeclaration` automatically. The macro extracts the tool name and description from the function, and derives the input schema from the argument type.

**Crate structure:**
```
rust2_tool_macros/    (proc-macro crate)
├── src/
│   ├── lib.rs        (proc macro definitions)
│   └── codegen.rs    (code generation logic)
```

**Macro API:**

```rust
// Args struct just needs JsonSchema derivation (uses doc comments for field descriptions)
#[derive(Deserialize, JsonSchema)]
struct CalculatorArgs { ... }

// Tool metadata on the function (name defaults to function name)
#[tool(description = "Perform basic arithmetic operations")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Or with explicit name override:
#[tool(name = "calc", description = "Perform basic arithmetic operations")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}
```

The `#[tool]` macro would expand to:

```rust
// Original function preserved
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Generated companion function to get ToolDeclaration
pub fn calculator_tool_declaration() -> ToolDeclaration {
    ToolDeclaration {
        name: "calculator".to_string(),
        description: "Perform basic arithmetic operations".to_string(),
        input_schema: serde_json::to_value(&schema_for!(CalculatorArgs))
            .expect("Failed to serialize schema"),
    }
}
```

**How it works:**
1. Macro parses function signature to extract first parameter type (`CalculatorArgs`)
2. Macro reads `name` attribute (or defaults to function name)
3. Macro reads `description` attribute (required)
4. Generated code uses `schema_for!(CalculatorArgs)` to create input schema
5. Parameter descriptions come from doc comments on the struct fields

### Phase 3: Enhanced Tool Attributes

Support additional attributes for fine-grained control on functions:

```rust
#[tool(
    name = "custom_name",           // Override function name
    description = "...",            // Tool description (required)
    example = "...",                // Optional usage example
)]
async fn my_tool(args: Args) -> Result<Output, String> { ... }
```

Field-level documentation and constraints use standard approaches:

```rust
#[derive(Deserialize, JsonSchema)]
struct Args {
    /// Description for the field (shown in schema)
    field: String,

    /// Percentage value between 0 and 100
    #[schemars(range(min = 0, max = 100))]
    percentage: u8,

    /// Optional email address
    #[schemars(regex = r"^[^@]+@[^@]+\.[^@]+$")]
    email: Option<String>,
}
```

### Phase 4: Integrated Registration

The `#[tool]` macro generates metadata that makes registration seamless. Here are the proposed approaches:

**Option A: Tool module pattern (recommended)**

The macro generates a module containing all tool metadata:

```rust
// User writes:
#[tool(description = "Perform basic arithmetic operations")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Macro generates:
pub mod calculator_tool {
    pub const NAME: &'static str = "calculator";

    pub fn declaration() -> ToolDeclaration {
        // Generated declaration
    }

    pub use super::calculator as execute;
}
```

Then the registry can have a convenience method:

```rust
impl FunctionRegistry {
    /// Register a tool using its generated module
    pub fn register_tool<F>(&mut self, name: &'static str, func: F, declaration: ToolDeclaration) -> ToolDeclaration
    where
        F: /* async function trait bounds */
    {
        self.register_async(name, func);
        declaration
    }
}

// Usage - super clean!
let mut registry = FunctionRegistry::new();
let tool_declarations = vec![
    registry.register_tool(
        calculator_tool::NAME,
        calculator_tool::execute,
        calculator_tool::declaration()
    ),
];
```

**Even simpler with a helper macro:**

```rust
macro_rules! register_tools {
    ($registry:expr, $($tool_mod:path),+ $(,)?) => {
        vec![
            $(
                {
                    $registry.register_async($tool_mod::NAME, $tool_mod::execute);
                    $tool_mod::declaration()
                }
            ),+
        ]
    };
}

// Usage:
let mut registry = FunctionRegistry::new();
let tool_declarations = register_tools!(registry, calculator_tool, another_tool);
```

**Option B: Simpler companion function (less magic)**

The macro just generates a companion function:

```rust
// User writes:
#[tool(description = "Perform basic arithmetic operations")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Macro generates:
pub fn calculator_tool_declaration() -> ToolDeclaration {
    ToolDeclaration {
        name: "calculator".to_string(),
        description: "Perform basic arithmetic operations".to_string(),
        input_schema: serde_json::to_value(&schema_for!(CalculatorArgs)).unwrap(),
    }
}

pub const CALCULATOR_TOOL_NAME: &str = "calculator";
```

Then use a helper macro or explicit registration:

```rust
// Manual:
registry.register_async(CALCULATOR_TOOL_NAME, calculator);
let tool_declarations = vec![calculator_tool_declaration()];

// Or with helper macro:
macro_rules! register_tool {
    ($registry:expr, $tool:ident) => {{
        let name_const = concat!(stringify!($tool), "_TOOL_NAME");
        let decl_fn = concat!(stringify!($tool), "_tool_declaration");
        $registry.register_async($name_const, $tool);
        decl_fn()
    }};
}
```

**Option C: Auto-registration via inventory (future enhancement)**

```rust
#[tool(auto_register, description = "...")]
async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    // Implementation
}

// Generates static registration that can be discovered at runtime
inventory::submit! {
    ToolRegistration::new("calculator", calculator, calculator_tool_declaration())
}
```

**Recommended approach:** Start with Option B (simpler, explicit), then potentially add the module pattern (Option A) if the ergonomics prove valuable in practice.

## Implementation Plan

### Step 1: Add schemars dependency
- Add `schemars` to Cargo.toml
- Update existing arg structs to derive `JsonSchema`
- Test schema generation manually

### Step 2: Create manual helper functions
Before building macros, create helper functions to validate the approach:

```rust
// In src/llm/tools/declaration.rs

pub fn create_tool_declaration<T: JsonSchema>(
    name: impl Into<String>,
    description: impl Into<String>,
) -> ToolDeclaration {
    let schema = schema_for!(T);
    ToolDeclaration {
        name: name.into(),
        description: description.into(),
        input_schema: serde_json::to_value(&schema)
            .expect("Failed to serialize schema"),
    }
}
```

Usage:
```rust
let decl = create_tool_declaration::<CalculatorArgs>(
    "calculator",
    "Perform basic arithmetic operations"
);
```

### Step 3: Create proc-macro crate
- Set up `rust2_tool_macros` crate
- Implement basic `#[tool]` attribute macro
- Test with simple examples

### Step 4: Implement struct metadata
- Add `#[tool(...)]` attributes for structs
- Generate trait implementations for metadata
- Test schema generation with attributes

### Step 5: Implement function macro
- Parse function signature
- Extract argument type
- Generate `tool_declaration()` method
- Test with various function signatures

### Step 6: Enhance FunctionRegistry
- Add `register_tool()` method
- Support automatic declaration management
- Update Agent to use declarations from registry

### Step 7: Update examples
- Refactor `agent_calculator.rs` to use new macros
- Create example showing advanced features
- Document migration path

### Step 8: Documentation
- Write comprehensive API docs
- Create migration guide
- Add troubleshooting section

## Technical Considerations

### Type Mapping

Ensure proper mapping between Rust types and JSON Schema types:

| Rust Type | JSON Schema Type | Notes |
|-----------|------------------|-------|
| `String` | `string` | |
| `i32`, `i64`, `u32`, `u64` | `integer` | |
| `f32`, `f64` | `number` | |
| `bool` | `boolean` | |
| `Vec<T>` | `array` with items | |
| `Option<T>` | Allow null or omit required | |
| `HashMap<String, T>` | `object` with additionalProperties | |
| Enums | `enum` or `string` enum | Based on representation |
| Structs | `object` | |

### Error Handling

Macro should provide helpful error messages:
- Missing required attributes
- Invalid attribute values
- Type constraints not supported
- Unsupported function signatures

### Async Function Support

Handle both sync and async functions:
```rust
#[tool]
fn sync_calculator(args: Args) -> Result<Output, String> { ... }

#[tool]
async fn async_calculator(args: Args) -> Result<Output, String> { ... }
```

### Validation

Add compile-time checks where possible:
- Args must implement `Deserialize`
- Output must implement `Serialize`
- Return type must be `Result<T, E>`

## Alternative Approaches Considered

### 1. Reflection-based (runtime)
**Rejected**: Rust doesn't have runtime reflection, would require significant overhead

### 2. Build script code generation
**Rejected**: Less ergonomic, requires separate build step, harder to debug

### 3. Inventory-based auto-registration
**Future consideration**: Could be added in Phase 5 for automatic tool discovery

### 4. Type-state pattern
**Too complex**: While type-safe, adds complexity for minimal benefit

## Migration Strategy

### Backwards Compatibility

Old approach continues to work:
```rust
// Still supported
registry.register_async("calculator", calculator);
let declarations = vec![ToolDeclaration { ... }];
```

### Gradual Migration

1. Update structs to derive `JsonSchema`
2. Use helper function to generate declarations
3. Apply `#[tool]` macro to functions
4. Remove manual declarations

### Example Migration

**Before:**
```rust
registry.register_async("calculator", calculator);
let tool_declarations = vec![ToolDeclaration { /* 30 lines */ }];
```

**After:**
```rust
registry.register_tool(calculator);
// Declaration auto-generated from #[tool] macro
```

## Testing Strategy

### Unit Tests
- Schema generation for primitive types
- Schema generation for complex types
- Attribute parsing
- Code generation correctness

### Integration Tests
- End-to-end tool registration
- Agent execution with derived tools
- Error handling

### Example Tests
- Update existing examples to use macros
- Create new examples showing advanced features

## Success Criteria

1. ✅ Reduce boilerplate by 50%+ for typical tools
2. ✅ Maintain type safety - schemas match struct definitions
3. ✅ Support all existing tool patterns
4. ✅ Clear error messages for common mistakes
5. ✅ Zero-cost abstractions (no runtime overhead)
6. ✅ Backwards compatible with existing code

## Future Enhancements

### Phase 5: Advanced Features
- Custom validators
- Parameter constraints (min/max, regex patterns)
- Conditional fields (if X then require Y)
- Multiple parameter sets (method overloading)

### Phase 6: IDE Support
- Rust-analyzer hints for generated code
- Auto-completion for tool attributes
- Inline documentation

### Phase 7: Tool Discovery
- Automatic scanning for `#[tool]` functions
- Runtime tool registry population
- Dynamic tool loading

## Open Questions

1. **Naming**: Should we use `#[tool]` or something more specific like `#[llm_tool]`?
2. **Error types**: Should we enforce a specific error type or allow flexibility?
3. **Registry**: Should registry be global or per-agent?
4. **Versioning**: How to handle tool schema versioning?
5. **Testing**: Should we auto-generate test cases from schemas?

## References

- [schemars crate](https://docs.rs/schemars/)
- [JSON Schema specification](https://json-schema.org/)
- [Rust procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html)
- [Anthropic Claude tool use](https://docs.anthropic.com/claude/docs/tool-use)
- [OpenAI function calling](https://platform.openai.com/docs/guides/function-calling)
