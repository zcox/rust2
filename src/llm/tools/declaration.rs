//! Tool declaration helpers using JSON Schema generation

use schemars::{schema_for, JsonSchema};

use crate::llm::core::types::ToolDeclaration;

/// Create a tool declaration from a type that implements JsonSchema
///
/// This is a helper function to automatically generate the input schema
/// from a Rust type using the schemars crate.
///
/// # Example
///
/// ```ignore
/// use schemars::JsonSchema;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, JsonSchema)]
/// struct CalculatorArgs {
///     /// The operation to perform
///     operation: String,
///     /// First operand
///     a: f64,
///     /// Second operand
///     b: f64,
/// }
///
/// let decl = create_tool_declaration::<CalculatorArgs>(
///     "calculator",
///     "Perform basic arithmetic operations"
/// );
/// ```
pub fn create_tool_declaration<T: JsonSchema>(
    name: impl Into<String>,
    description: impl Into<String>,
) -> ToolDeclaration {
    let schema = schema_for!(T);
    ToolDeclaration {
        name: name.into(),
        description: description.into(),
        input_schema: serde_json::to_value(&schema)
            .expect("Failed to serialize schema - this is a bug in schemars or the JsonSchema impl"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, JsonSchema)]
    struct TestArgs {
        /// A string field
        field1: String,
        /// A number field
        field2: f64,
    }

    #[test]
    fn test_create_tool_declaration() {
        let decl = create_tool_declaration::<TestArgs>("test_tool", "A test tool");

        assert_eq!(decl.name, "test_tool");
        assert_eq!(decl.description, "A test tool");

        // Verify the schema is valid JSON
        assert!(decl.input_schema.is_object());

        // Verify it has the expected structure
        let schema_obj = decl.input_schema.as_object().unwrap();
        assert!(schema_obj.contains_key("$schema"));
        assert!(schema_obj.contains_key("title"));
        assert!(schema_obj.contains_key("type"));
        assert!(schema_obj.contains_key("properties"));
    }

    #[test]
    fn test_schema_includes_doc_comments() {
        let decl = create_tool_declaration::<TestArgs>("test", "test");

        let schema_str = serde_json::to_string_pretty(&decl.input_schema).unwrap();

        // Doc comments should appear as descriptions in the schema
        assert!(schema_str.contains("A string field"));
        assert!(schema_str.contains("A number field"));
    }
}
