//! Mapping between abstraction types and Claude-specific types

use crate::llm::core::types::{
    ContentBlock, ContentBlockStart, ContentDelta, FinishReason, GenerateRequest, Message,
    MessageMetadata, MessageRole, PartialToolUse, StreamEvent, ToolDeclaration, UsageMetadata,
};

use super::types::{
    ClaudeContent, ClaudeContentBlock, ClaudeContentBlockStart, ClaudeContentDelta,
    ClaudeMessage, ClaudeStreamEvent, ClaudeTool, StreamRawPredictRequest,
};

/// Convert our abstraction request to Claude's request format
pub fn to_claude_request(request: GenerateRequest) -> StreamRawPredictRequest {
    StreamRawPredictRequest {
        anthropic_version: "vertex-2023-10-16".to_string(),
        max_tokens: request.config.max_tokens,
        messages: request
            .messages
            .into_iter()
            .map(to_claude_message)
            .collect(),
        system: request.system,
        tools: request.tools.map(|tools| {
            tools
                .into_iter()
                .map(to_claude_tool)
                .collect()
        }),
        temperature: request.config.temperature,
        top_p: request.config.top_p,
        stop_sequences: request.config.stop_sequences,
        stream: true,
    }
}

/// Convert our Message to Claude's ClaudeMessage
fn to_claude_message(message: Message) -> ClaudeMessage {
    let role = match message.role {
        MessageRole::User => "user".to_string(),
        MessageRole::Assistant => "assistant".to_string(),
        MessageRole::Tool => "user".to_string(), // Tool results go in user messages for Claude
    };

    // If there's only one text block, use simple text content
    if message.content.len() == 1 {
        if let ContentBlock::Text { text } = &message.content[0] {
            return ClaudeMessage {
                role,
                content: ClaudeContent::Text(text.clone()),
            };
        }
    }

    // Otherwise, use blocks
    let blocks = message
        .content
        .into_iter()
        .map(to_claude_content_block)
        .collect();

    ClaudeMessage {
        role,
        content: ClaudeContent::Blocks(blocks),
    }
}

/// Convert our ContentBlock to Claude's ClaudeContentBlock
fn to_claude_content_block(block: ContentBlock) -> ClaudeContentBlock {
    match block {
        ContentBlock::Text { text } => ClaudeContentBlock::Text { text },
        ContentBlock::ToolUse { id, name, input } => ClaudeContentBlock::ToolUse { id, name, input },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => ClaudeContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error: if is_error { Some(true) } else { None },
        },
    }
}

/// Convert our ToolDeclaration to Claude's ClaudeTool
fn to_claude_tool(tool: ToolDeclaration) -> ClaudeTool {
    ClaudeTool {
        name: tool.name,
        description: tool.description,
        input_schema: tool.input_schema,
    }
}

/// Convert Claude's stream event to our abstraction's StreamEvent
/// Returns a vector of events because some Claude events may need to be split
pub fn from_claude_event(
    event: ClaudeStreamEvent,
    accumulated_usage: &mut UsageMetadata,
) -> Vec<StreamEvent> {
    match event {
        ClaudeStreamEvent::MessageStart { message } => {
            // Update accumulated usage with initial tokens
            accumulated_usage.input_tokens = message.usage.input_tokens;
            accumulated_usage.output_tokens = message.usage.output_tokens;
            accumulated_usage.total_tokens =
                accumulated_usage.input_tokens + accumulated_usage.output_tokens;

            vec![StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: message.id,
                    role: MessageRole::Assistant,
                    usage: Some(*accumulated_usage),
                },
            }]
        }
        ClaudeStreamEvent::ContentBlockStart {
            index,
            content_block,
        } => {
            let block = match content_block {
                ClaudeContentBlockStart::Text { text } => ContentBlockStart::Text { text },
                ClaudeContentBlockStart::ToolUse { id, name } => {
                    ContentBlockStart::ToolUse { id, name }
                }
            };

            vec![StreamEvent::ContentBlockStart { index, block }]
        }
        ClaudeStreamEvent::ContentBlockDelta { index, delta } => {
            let content_delta = match delta {
                ClaudeContentDelta::TextDelta { text } => ContentDelta::TextDelta { text },
                ClaudeContentDelta::InputJsonDelta { partial_json } => {
                    ContentDelta::ToolUseDelta {
                        partial: PartialToolUse {
                            id: None,
                            name: None,
                            partial_json,
                        },
                    }
                }
            };

            vec![StreamEvent::ContentDelta {
                index,
                delta: content_delta,
            }]
        }
        ClaudeStreamEvent::ContentBlockStop { index } => {
            vec![StreamEvent::ContentBlockEnd { index }]
        }
        ClaudeStreamEvent::MessageDelta { delta, usage } => {
            // Update accumulated usage if provided
            if let Some(usage) = usage {
                accumulated_usage.output_tokens = usage.output_tokens;
                accumulated_usage.total_tokens =
                    accumulated_usage.input_tokens + accumulated_usage.output_tokens;
            }

            // If there's a stop reason, this is the final delta
            if let Some(stop_reason) = delta.stop_reason {
                let finish_reason = match stop_reason.as_str() {
                    "end_turn" => FinishReason::EndTurn,
                    "max_tokens" => FinishReason::MaxTokens,
                    "stop_sequence" => FinishReason::StopSequence,
                    "tool_use" => FinishReason::ToolUse,
                    other => FinishReason::Other(other.to_string()),
                };

                vec![StreamEvent::MessageEnd {
                    finish_reason,
                    usage: *accumulated_usage,
                }]
            } else {
                // Just a usage update
                vec![StreamEvent::MessageDelta {
                    usage: Some(*accumulated_usage),
                }]
            }
        }
        ClaudeStreamEvent::MessageStop => {
            // This event typically comes after MessageDelta with stop_reason
            // We can emit it as a no-op or skip it
            vec![]
        }
        ClaudeStreamEvent::Ping => {
            // Keep-alive event, skip
            vec![]
        }
        ClaudeStreamEvent::Error { error } => {
            vec![StreamEvent::Error {
                error: format!("{}: {}", error.error_type, error.message),
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::core::config::GenerationConfig;

    #[test]
    fn test_to_claude_request_basic() {
        let request = GenerateRequest {
            messages: vec![Message::user("Hello")],
            tools: None,
            config: GenerationConfig {
                max_tokens: 1024,
                temperature: Some(0.7),
                top_p: Some(0.9),
                top_k: None,
                stop_sequences: None,
            },
            system: Some("You are helpful".to_string()),
        };

        let claude_request = to_claude_request(request);

        assert_eq!(claude_request.anthropic_version, "vertex-2023-10-16");
        assert_eq!(claude_request.max_tokens, 1024);
        assert_eq!(claude_request.temperature, Some(0.7));
        assert_eq!(claude_request.top_p, Some(0.9));
        assert_eq!(claude_request.system, Some("You are helpful".to_string()));
        assert!(claude_request.stream);
        assert_eq!(claude_request.messages.len(), 1);
    }

    #[test]
    fn test_to_claude_message_simple_text() {
        let message = Message::user("Hello");
        let claude_message = to_claude_message(message);

        assert_eq!(claude_message.role, "user");
        match claude_message.content {
            ClaudeContent::Text(text) => assert_eq!(text, "Hello"),
            _ => panic!("Expected simple text content"),
        }
    }

    #[test]
    fn test_to_claude_message_with_tool_use() {
        let message = Message {
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me check".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "get_weather".to_string(),
                    input: serde_json::json!({"location": "SF"}),
                },
            ],
        };

        let claude_message = to_claude_message(message);
        assert_eq!(claude_message.role, "assistant");

        match claude_message.content {
            ClaudeContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                match &blocks[0] {
                    ClaudeContentBlock::Text { text } => assert_eq!(text, "Let me check"),
                    _ => panic!("Expected text block"),
                }
                match &blocks[1] {
                    ClaudeContentBlock::ToolUse { id, name, .. } => {
                        assert_eq!(id, "tool-1");
                        assert_eq!(name, "get_weather");
                    }
                    _ => panic!("Expected tool use block"),
                }
            }
            _ => panic!("Expected blocks content"),
        }
    }

    #[test]
    fn test_to_claude_message_tool_result() {
        let message = Message::tool_result("tool-1", "72°F");
        let claude_message = to_claude_message(message);

        // Tool results become user messages in Claude
        assert_eq!(claude_message.role, "user");

        match claude_message.content {
            ClaudeContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ClaudeContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id, "tool-1");
                        assert_eq!(content, "72°F");
                        assert_eq!(*is_error, None);
                    }
                    _ => panic!("Expected tool result block"),
                }
            }
            _ => panic!("Expected blocks content"),
        }
    }

    #[test]
    fn test_to_claude_tool() {
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

        let claude_tool = to_claude_tool(tool);
        assert_eq!(claude_tool.name, "get_weather");
        assert_eq!(claude_tool.description, "Get weather");
    }

    #[test]
    fn test_from_claude_event_message_start() {
        use super::super::types::{ClaudeMessageData, ClaudeUsage};

        let event = ClaudeStreamEvent::MessageStart {
            message: ClaudeMessageData {
                id: "msg_123".to_string(),
                message_type: "message".to_string(),
                role: "assistant".to_string(),
                content: vec![],
                model: "claude-sonnet-4-5".to_string(),
                stop_reason: None,
                stop_sequence: None,
                usage: ClaudeUsage {
                    input_tokens: 10,
                    output_tokens: 0,
                },
            },
        };

        let mut accumulated_usage = UsageMetadata::new(0, 0);
        let events = from_claude_event(event, &mut accumulated_usage);

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_123");
                assert_eq!(message.role, MessageRole::Assistant);
                assert_eq!(message.usage.unwrap().input_tokens, 10);
            }
            _ => panic!("Expected MessageStart event"),
        }
        assert_eq!(accumulated_usage.input_tokens, 10);
    }

    #[test]
    fn test_from_claude_event_content_block_start_text() {
        let event = ClaudeStreamEvent::ContentBlockStart {
            index: 0,
            content_block: ClaudeContentBlockStart::Text {
                text: "".to_string(),
            },
        };

        let mut usage = UsageMetadata::new(0, 0);
        let events = from_claude_event(event, &mut usage);

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentBlockStart { index, block } => {
                assert_eq!(*index, 0);
                match block {
                    ContentBlockStart::Text { text } => assert_eq!(text, ""),
                    _ => panic!("Expected text block"),
                }
            }
            _ => panic!("Expected ContentBlockStart event"),
        }
    }

    #[test]
    fn test_from_claude_event_content_delta_text() {
        let event = ClaudeStreamEvent::ContentBlockDelta {
            index: 0,
            delta: ClaudeContentDelta::TextDelta {
                text: "Hello".to_string(),
            },
        };

        let mut usage = UsageMetadata::new(0, 0);
        let events = from_claude_event(event, &mut usage);

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ContentDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentDelta::TextDelta { text } => assert_eq!(text, "Hello"),
                    _ => panic!("Expected text delta"),
                }
            }
            _ => panic!("Expected ContentDelta event"),
        }
    }

    #[test]
    fn test_from_claude_event_message_delta_with_stop_reason() {
        use super::super::types::{ClaudeMessageDeltaData, ClaudeUsage};

        let event = ClaudeStreamEvent::MessageDelta {
            delta: ClaudeMessageDeltaData {
                stop_reason: Some("end_turn".to_string()),
                stop_sequence: None,
            },
            usage: Some(ClaudeUsage {
                input_tokens: 10,
                output_tokens: 25,
            }),
        };

        let mut accumulated_usage = UsageMetadata::new(10, 0);
        let events = from_claude_event(event, &mut accumulated_usage);

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::MessageEnd {
                finish_reason,
                usage,
            } => {
                assert_eq!(*finish_reason, FinishReason::EndTurn);
                assert_eq!(usage.output_tokens, 25);
                assert_eq!(usage.total_tokens, 35);
            }
            _ => panic!("Expected MessageEnd event"),
        }
    }

    #[test]
    fn test_finish_reason_mapping() {
        use super::super::types::{ClaudeMessageDeltaData, ClaudeUsage};

        let test_cases = vec![
            ("end_turn", FinishReason::EndTurn),
            ("max_tokens", FinishReason::MaxTokens),
            ("stop_sequence", FinishReason::StopSequence),
            ("tool_use", FinishReason::ToolUse),
        ];

        for (claude_reason, expected_reason) in test_cases {
            let event = ClaudeStreamEvent::MessageDelta {
                delta: ClaudeMessageDeltaData {
                    stop_reason: Some(claude_reason.to_string()),
                    stop_sequence: None,
                },
                usage: Some(ClaudeUsage {
                    input_tokens: 10,
                    output_tokens: 20,
                }),
            };

            let mut usage = UsageMetadata::new(10, 0);
            let events = from_claude_event(event, &mut usage);

            match &events[0] {
                StreamEvent::MessageEnd { finish_reason, .. } => {
                    assert_eq!(*finish_reason, expected_reason);
                }
                _ => panic!("Expected MessageEnd event"),
            }
        }
    }
}
