//! Mapping between abstraction types and Gemini types

use uuid::Uuid;

use crate::llm::core::{
    config::GenerationConfig,
    types::{
        ContentBlock, ContentBlockStart, ContentDelta, FinishReason, GenerateRequest, Message,
        MessageMetadata, MessageRole, PartialToolUse, StreamEvent, ToolDeclaration, UsageMetadata,
    },
};

use super::types::{
    Content, FunctionCall, FunctionDeclaration, FunctionResponse,
    GeminiGenerationConfig, GenerateContentRequest, GenerateContentResponse, Part,
    SystemInstruction, Tool,
};

/// Convert our abstraction request to Gemini's request format
pub fn to_gemini_request(request: GenerateRequest) -> GenerateContentRequest {
    GenerateContentRequest {
        contents: request.messages.into_iter().map(to_gemini_content).collect(),
        system_instruction: request.system.map(|s| SystemInstruction {
            parts: vec![Part::Text { text: s }],
        }),
        tools: request.tools.map(|tools| {
            vec![Tool {
                function_declarations: tools.into_iter().map(to_gemini_function_declaration).collect(),
            }]
        }),
        generation_config: Some(to_gemini_generation_config(request.config)),
    }
}

/// Convert a message to Gemini's content format
fn to_gemini_content(message: Message) -> Content {
    let role = match message.role {
        MessageRole::User => "user".to_string(),
        MessageRole::Assistant => "model".to_string(),
        // Tool results are included in user role with FunctionResponse parts
        MessageRole::Tool => "user".to_string(),
    };

    let parts = message
        .content
        .into_iter()
        .map(to_gemini_part)
        .collect();

    Content { role, parts }
}

/// Convert a content block to a Gemini part
fn to_gemini_part(block: ContentBlock) -> Part {
    match block {
        ContentBlock::Text { text } => Part::Text { text },
        ContentBlock::ToolUse { id: _, name, input } => {
            // Note: Gemini doesn't use tool_use_id in the request, it's only in responses
            Part::FunctionCall {
                function_call: FunctionCall {
                    name,
                    args: input,
                },
            }
        }
        ContentBlock::ToolResult {
            tool_use_id: _,
            content,
            is_error,
        } => {
            // Extract the name from the content if it was stored, otherwise use a placeholder
            // In practice, the application needs to track which tool_use_id maps to which name
            // For now, we'll encode the result as a JSON object
            let response = if is_error {
                serde_json::json!({
                    "error": content
                })
            } else {
                // Try to parse content as JSON, otherwise wrap it
                serde_json::from_str(&content).unwrap_or_else(|_| {
                    serde_json::json!({
                        "result": content
                    })
                })
            };

            Part::FunctionResponse {
                function_response: FunctionResponse {
                    // Note: We need the function name here, but it's not in ToolResult.
                    // This is a limitation - the application must provide this context.
                    // For now, we'll use a placeholder. Real implementation would need
                    // to track the mapping from tool_use_id to function name.
                    name: "function".to_string(),
                    response,
                },
            }
        }
    }
}

/// Convert a tool declaration to Gemini's function declaration
fn to_gemini_function_declaration(tool: ToolDeclaration) -> FunctionDeclaration {
    FunctionDeclaration {
        name: tool.name,
        description: tool.description,
        parameters: tool.input_schema,
    }
}

/// Convert generation config to Gemini's format
fn to_gemini_generation_config(config: GenerationConfig) -> GeminiGenerationConfig {
    GeminiGenerationConfig {
        max_output_tokens: Some(config.max_tokens),
        temperature: config.temperature,
        top_p: config.top_p,
        top_k: config.top_k,
        stop_sequences: config.stop_sequences,
    }
}

/// Convert Gemini response to our abstraction's stream events
///
/// This function processes a Gemini response chunk and emits the appropriate stream events.
/// It maintains state about which content blocks have been seen to properly emit start/delta/end events.
pub fn from_gemini_response(
    response: GenerateContentResponse,
    current_index: &mut usize,
) -> Vec<StreamEvent> {
    let mut events = Vec::new();

    if response.candidates.is_empty() {
        return events;
    }

    let candidate = &response.candidates[0];

    // Process each part in the content
    for part in &candidate.content.parts {
        match part {
            Part::Text { text } => {
                // For text, we emit delta events
                events.push(StreamEvent::ContentDelta {
                    index: *current_index,
                    delta: ContentDelta::TextDelta { text: text.clone() },
                });
            }
            Part::FunctionCall { function_call } => {
                // For function calls, emit start and delta events
                // First time seeing this function call at this index
                events.push(StreamEvent::ContentBlockStart {
                    index: *current_index,
                    block: ContentBlockStart::ToolUse {
                        id: Uuid::new_v4().to_string(), // Generate ID since Gemini doesn't provide one
                        name: function_call.name.clone(),
                    },
                });

                // Emit the complete arguments as a delta
                events.push(StreamEvent::ContentDelta {
                    index: *current_index,
                    delta: ContentDelta::ToolUseDelta {
                        partial: PartialToolUse {
                            id: None,
                            name: Some(function_call.name.clone()),
                            partial_json: function_call.args.to_string(),
                        },
                    },
                });

                events.push(StreamEvent::ContentBlockEnd {
                    index: *current_index,
                });

                *current_index += 1;
            }
            Part::FunctionResponse { .. } => {
                // Function responses are not expected in model output
                // They're only in the request
            }
        }
    }

    // Handle finish reason and usage metadata
    if let Some(finish_reason_str) = &candidate.finish_reason {
        let finish_reason = map_finish_reason(finish_reason_str);

        if let Some(usage) = &response.usage_metadata {
            events.push(StreamEvent::MessageEnd {
                finish_reason,
                usage: UsageMetadata {
                    input_tokens: usage.prompt_token_count,
                    output_tokens: usage.candidates_token_count,
                    total_tokens: usage.total_token_count,
                },
            });
        } else {
            // If no usage metadata, create a zero usage
            events.push(StreamEvent::MessageEnd {
                finish_reason,
                usage: UsageMetadata {
                    input_tokens: 0,
                    output_tokens: 0,
                    total_tokens: 0,
                },
            });
        }
    }

    events
}

/// Map Gemini's finish reason to our abstraction
fn map_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "STOP" => FinishReason::Stop,
        "MAX_TOKENS" => FinishReason::MaxTokens,
        "SAFETY" => FinishReason::Safety,
        "RECITATION" => FinishReason::Other("Recitation".to_string()),
        other => FinishReason::Other(other.to_string()),
    }
}

/// Helper to create initial message start event
pub fn create_message_start(message_id: String) -> StreamEvent {
    StreamEvent::MessageStart {
        message: MessageMetadata {
            id: message_id,
            role: MessageRole::Assistant,
            usage: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::gemini::types::Candidate;

    #[test]
    fn test_to_gemini_content_user() {
        let message = Message::user("Hello");
        let content = to_gemini_content(message);
        assert_eq!(content.role, "user");
        assert_eq!(content.parts.len(), 1);
        match &content.parts[0] {
            Part::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_to_gemini_content_assistant() {
        let message = Message::assistant("Hi there");
        let content = to_gemini_content(message);
        assert_eq!(content.role, "model");
    }

    #[test]
    fn test_to_gemini_function_declaration() {
        let tool = ToolDeclaration {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        };
        let func_decl = to_gemini_function_declaration(tool);
        assert_eq!(func_decl.name, "get_weather");
        assert_eq!(func_decl.description, "Get weather");
    }

    #[test]
    fn test_to_gemini_generation_config() {
        let config = GenerationConfig::new(2048)
            .with_temperature(0.7)
            .with_top_k(40);
        let gemini_config = to_gemini_generation_config(config);
        assert_eq!(gemini_config.max_output_tokens, Some(2048));
        assert_eq!(gemini_config.temperature, Some(0.7));
        assert_eq!(gemini_config.top_k, Some(40));
    }

    #[test]
    fn test_map_finish_reason() {
        assert_eq!(map_finish_reason("STOP"), FinishReason::Stop);
        assert_eq!(map_finish_reason("MAX_TOKENS"), FinishReason::MaxTokens);
        assert_eq!(map_finish_reason("SAFETY"), FinishReason::Safety);
        assert_eq!(
            map_finish_reason("RECITATION"),
            FinishReason::Other("Recitation".to_string())
        );
        assert_eq!(
            map_finish_reason("UNKNOWN"),
            FinishReason::Other("UNKNOWN".to_string())
        );
    }

    #[test]
    fn test_from_gemini_response_text() {
        let response = GenerateContentResponse {
            candidates: vec![Candidate {
                content: Content {
                    role: "model".to_string(),
                    parts: vec![Part::Text {
                        text: "Hello!".to_string(),
                    }],
                },
                finish_reason: None,
                safety_ratings: None,
            }],
            usage_metadata: None,
        };

        let mut index = 0;
        let events = from_gemini_response(response, &mut index);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentDelta { delta, .. } => match delta {
                ContentDelta::TextDelta { text } => assert_eq!(text, "Hello!"),
                _ => panic!("Expected text delta"),
            },
            _ => panic!("Expected content delta"),
        }
    }

    #[test]
    fn test_from_gemini_response_with_finish() {
        let response = GenerateContentResponse {
            candidates: vec![Candidate {
                content: Content {
                    role: "model".to_string(),
                    parts: vec![Part::Text {
                        text: "Done".to_string(),
                    }],
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: None,
            }],
            usage_metadata: Some(super::super::types::UsageMetadata {
                prompt_token_count: 10,
                candidates_token_count: 5,
                total_token_count: 15,
            }),
        };

        let mut index = 0;
        let events = from_gemini_response(response, &mut index);
        assert_eq!(events.len(), 2); // Delta + MessageEnd
        match &events[1] {
            StreamEvent::MessageEnd { finish_reason, usage } => {
                assert_eq!(*finish_reason, FinishReason::Stop);
                assert_eq!(usage.total_tokens, 15);
            }
            _ => panic!("Expected message end"),
        }
    }

    #[test]
    fn test_to_gemini_request_with_tools() {
        let request = GenerateRequest {
            messages: vec![Message::user("What's the weather?")],
            tools: Some(vec![ToolDeclaration {
                name: "get_weather".to_string(),
                description: "Get weather".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            }]),
            config: GenerationConfig::default(),
            system: Some("You are helpful".to_string()),
        };

        let gemini_request = to_gemini_request(request);
        assert!(gemini_request.system_instruction.is_some());
        assert!(gemini_request.tools.is_some());
        let tools = gemini_request.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function_declarations.len(), 1);
        assert_eq!(tools[0].function_declarations[0].name, "get_weather");
    }
}
