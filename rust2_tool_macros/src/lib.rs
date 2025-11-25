//! Procedural macros for automatic tool declaration generation

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, Expr, ExprLit, ItemFn, Lit, Meta, Type};

/// Attribute macro to automatically generate tool declarations from functions
///
/// # Example
///
/// ```ignore
/// #[tool(description = "Perform basic arithmetic operations")]
/// async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
///     // Implementation
/// }
/// ```
///
/// This will generate a module `calculator_tool` containing:
/// - `NAME`: A constant with the tool name
/// - `declaration()`: Function returning the ToolDeclaration
/// - `execute`: Re-export of the original function
/// - `registration()`: Function returning a complete ToolRegistration for one-step registration
///
/// # Usage
///
/// ```ignore
/// // Simplest approach - one argument registration
/// registry.register(calculator_tool::registration())?;
///
/// // Or use the explicit methods
/// registry.register_async_tool(
///     calculator_tool::execute,
///     calculator_tool::declaration()
/// )?;
/// ```
///
/// # Attributes
///
/// - `description`: (required) Description of what the tool does
/// - `name`: (optional) Override the tool name (defaults to function name)
///
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments
    let attr_args = parse_macro_input!(attr with Punctuated::<Meta, Comma>::parse_terminated);

    // Parse the function
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract metadata from attributes
    let mut description = None;
    let mut tool_name = None;

    for arg in attr_args {
        match arg {
            Meta::NameValue(nv) => {
                if nv.path.is_ident("description") {
                    if let Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) = &nv.value {
                        description = Some(lit.value());
                    }
                } else if nv.path.is_ident("name") {
                    if let Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) = &nv.value {
                        tool_name = Some(lit.value());
                    }
                }
            }
            _ => {}
        }
    }

    // Description is required
    let description = match description {
        Some(d) => d,
        None => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "tool attribute requires a 'description' parameter"
            )
            .to_compile_error()
            .into();
        }
    };

    // Default to function name if not specified
    let fn_name = &input_fn.sig.ident;
    let tool_name = tool_name.unwrap_or_else(|| fn_name.to_string());

    // Extract the argument type from the first parameter
    let arg_type = match input_fn.sig.inputs.first() {
        Some(syn::FnArg::Typed(pat_type)) => &pat_type.ty,
        _ => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "tool function must have at least one parameter"
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate the module name: calculator -> calculator_tool
    let module_name = syn::Ident::new(
        &format!("{}_tool", fn_name),
        fn_name.span(),
    );

    // Strip any reference or path from the type to get the base type
    let base_type = strip_type_modifiers(arg_type);

    // Make the function public so it can be re-exported
    let mut pub_input_fn = input_fn.clone();
    pub_input_fn.vis = syn::parse_quote!(pub);

    // Check if the function is async or sync
    let is_async = input_fn.sig.asyncness.is_some();

    // Generate the wrapper logic for the registration() function
    let wrapper_logic = if is_async {
        // Async function wrapper
        quote! {
            let wrapper = move |args_json: serde_json::Value| {
                use futures::future::BoxFuture;

                // Deserialize arguments
                let args = match serde_json::from_value::<#base_type>(args_json) {
                    Ok(args) => args,
                    Err(e) => {
                        let err_msg = format!("Failed to deserialize arguments: {}", e);
                        return Box::pin(async move { Err(err_msg) }) as BoxFuture<'static, _>;
                    }
                };

                // Call the async function
                let future = execute(args);

                // Box the future and handle serialization
                Box::pin(async move {
                    match future.await {
                        Ok(result) => {
                            serde_json::to_string(&result)
                                .map_err(|e| format!("Failed to serialize result: {}", e))
                        }
                        Err(e) => Err(e),
                    }
                }) as BoxFuture<'static, _>
            };
        }
    } else {
        // Sync function wrapper
        quote! {
            let wrapper = move |args_json: serde_json::Value| {
                use futures::future::BoxFuture;

                // Deserialize arguments
                let args = match serde_json::from_value::<#base_type>(args_json) {
                    Ok(args) => args,
                    Err(e) => {
                        let err_msg = format!("Failed to deserialize arguments: {}", e);
                        return Box::pin(async move { Err(err_msg) }) as BoxFuture<'static, _>;
                    }
                };

                // Call the sync function
                let result = execute(args);

                // Box the result as a future
                Box::pin(async move {
                    match result {
                        Ok(result) => {
                            serde_json::to_string(&result)
                                .map_err(|e| format!("Failed to serialize result: {}", e))
                        }
                        Err(e) => Err(e),
                    }
                }) as BoxFuture<'static, _>
            };
        }
    };

    // Generate the output - creates a module with all tool metadata
    let output = quote! {
        // Original function (made pub for re-export)
        #pub_input_fn

        // Generated module containing all tool metadata
        #[allow(dead_code)]
        pub mod #module_name {
            use super::*;

            /// The name of this tool (use when registering)
            pub const NAME: &str = #tool_name;

            /// Get the ToolDeclaration for this tool
            pub fn declaration() -> rust2::llm::ToolDeclaration {
                rust2::llm::create_tool_declaration::<#base_type>(
                    #tool_name,
                    #description
                )
            }

            /// The executable function for this tool (re-exported from parent)
            pub use super::#fn_name as execute;

            /// Get a complete ToolRegistration for one-step registration
            ///
            /// This is the simplest way to register a tool:
            /// ```ignore
            /// registry.register(calculator_tool::registration())?;
            /// ```
            pub fn registration() -> rust2::llm::tools::ToolRegistration {
                #wrapper_logic

                rust2::llm::tools::ToolRegistration {
                    name: NAME,
                    function: Box::new(wrapper),
                    declaration: declaration(),
                }
            }
        }
    };

    TokenStream::from(output)
}

/// Strip reference and other modifiers from a type to get the base type
fn strip_type_modifiers(ty: &Type) -> &Type {
    match ty {
        Type::Reference(type_ref) => strip_type_modifiers(&type_ref.elem),
        Type::Path(_) => ty,
        _ => ty,
    }
}
