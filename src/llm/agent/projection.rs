//! Event → Message projection logic
//!
//! This module converts ThreadEvent streams back into LLM Message format.
//!
//! Key principles:
//! - Only "completed" events affect projection (UserMessageReceived, LlmResponseCompleted, ToolExecution*)
//! - Streaming events (deltas) are for real-time display only, not projection
//! - Projection is idempotent and deterministic
//! - Can rebuild entire conversation history from events

use super::events::{ContentBlockData, ThreadEvent};
use crate::llm::core::types::{ContentBlock, Message, MessageRole};

/// Project thread events into LLM messages
///
/// This function reconstructs the conversation history by processing only the
/// "completed" events and ignoring all streaming delta events.
///
/// # Example Event Sequence → Messages
///
/// ```text
/// Events:
///   1. UserMessageReceived("What's the weather in Tokyo?")
///   2. AgentIterationStarted(1)
///   3. LlmCallStarted(...)
///   4. LlmContentDelta("Let me") - IGNORED
///   5. LlmContentDelta(" check") - IGNORED
///   6. LlmResponseCompleted([Text("Let me check"), ToolUse(...)])
///   7. ToolExecutionStarted(...)
///   8. ToolExecutionCompleted("toolu_123", "{\"temp\": 18}")
///   9. AgentIterationStarted(2)
///   10. LlmResponseCompleted([Text("It's 18°C")])
///   11. AgentCompleted
///
/// Messages:
///   1. User: "What's the weather in Tokyo?"
///   2. Assistant: [Text("Let me check"), ToolUse(...)]
///   3. Tool: ToolResult("toolu_123", "{\"temp\": 18}")
///   4. Assistant: [Text("It's 18°C")]
/// ```
pub fn project_events_to_messages(events: &[ThreadEvent]) -> Vec<Message> {
    let mut messages = Vec::new();

    for event in events {
        match event {
            // User message received → Create user message
            ThreadEvent::UserMessageReceived(data) => {
                messages.push(Message {
                    role: MessageRole::User,
                    content: vec![ContentBlock::Text {
                        text: data.message.clone(),
                    }],
                });
            }

            // LLM response completed → Create assistant message with all content blocks
            ThreadEvent::LlmResponseCompleted(data) => {
                // Convert ContentBlockData to ContentBlock
                let content: Vec<ContentBlock> = data
                    .content_blocks
                    .iter()
                    .map(|block| match block {
                        ContentBlockData::Text { text } => {
                            ContentBlock::Text { text: text.clone() }
                        }
                        ContentBlockData::ToolUse { id, name, input } => ContentBlock::ToolCall {
                            tool_call_id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                    })
                    .collect();

                messages.push(Message {
                    role: MessageRole::Assistant,
                    content,
                });
            }

            // Tool execution completed → Create tool result message
            ThreadEvent::ToolExecutionCompleted(data) => {
                messages.push(Message {
                    role: MessageRole::Tool,
                    content: vec![ContentBlock::ToolResult {
                        tool_call_id: data.tool_use_id.clone(),
                        content: data.result.clone(),
                        is_error: false,
                    }],
                });
            }

            // Tool execution failed → Create tool error message
            ThreadEvent::ToolExecutionFailed(data) => {
                messages.push(Message {
                    role: MessageRole::Tool,
                    content: vec![ContentBlock::ToolResult {
                        tool_call_id: data.tool_use_id.clone(),
                        content: data.error.clone(),
                        is_error: true,
                    }],
                });
            }

            // All other events are ignored (streaming deltas, iteration markers, etc.)
            _ => {}
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::agent::events::*;
    use chrono::Utc;
    use serde_json::json;

    // =============================================================================
    // Simple Q&A Tests (no tools)
    // =============================================================================

    #[test]
    fn test_simple_qa_no_tools() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "What is 2+2?".to_string(),
                timestamp: now,
            }),
            ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
                iteration: 1,
                timestamp: now,
            }),
            ThreadEvent::LlmCallStarted(LlmCallStartedData {
                provider: "claude".to_string(),
                model: "sonnet".to_string(),
                message_count: 1,
                timestamp: now,
            }),
            // Streaming deltas (should be ignored)
            ThreadEvent::LlmContentDelta(LlmContentDeltaData {
                content_block_index: 0,
                delta_type: "text".to_string(),
                text: "2+2".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmContentDelta(LlmContentDeltaData {
                content_block_index: 0,
                delta_type: "text".to_string(),
                text: " equals 4".to_string(),
                timestamp: now,
            }),
            // Completed response
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "2+2 equals 4".to_string(),
                }],
                timestamp: now,
            }),
            ThreadEvent::AgentIterationCompleted(AgentIterationCompletedData {
                iteration: 1,
                has_tool_uses: false,
                timestamp: now,
            }),
            ThreadEvent::AgentCompleted(AgentCompletedData {
                total_iterations: 1,
                final_response: "2+2 equals 4".to_string(),
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        // Should have exactly 2 messages: user + assistant
        assert_eq!(messages.len(), 2);

        // Verify user message
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content.len(), 1);
        match &messages[0].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "What is 2+2?"),
            _ => panic!("Expected text content"),
        }

        // Verify assistant message
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].content.len(), 1);
        match &messages[1].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "2+2 equals 4"),
            _ => panic!("Expected text content"),
        }
    }

    // =============================================================================
    // Single Tool Use Tests
    // =============================================================================

    #[test]
    fn test_single_tool_use() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "What's the weather in Tokyo?".to_string(),
                timestamp: now,
            }),
            ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
                iteration: 1,
                timestamp: now,
            }),
            // LLM responds with tool use
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "tool_use".to_string(),
                content_blocks: vec![
                    ContentBlockData::Text {
                        text: "Let me check the weather for you.".to_string(),
                    },
                    ContentBlockData::ToolUse {
                        id: "toolu_123".to_string(),
                        name: "get_weather".to_string(),
                        input: json!({"location": "Tokyo", "unit": "celsius"}),
                    },
                ],
                timestamp: now,
            }),
            // Tool execution
            ThreadEvent::ToolExecutionStarted(ToolExecutionStartedData {
                tool_use_id: "toolu_123".to_string(),
                name: "get_weather".to_string(),
                input: json!({"location": "Tokyo", "unit": "celsius"}),
                timestamp: now,
            }),
            ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
                tool_use_id: "toolu_123".to_string(),
                name: "get_weather".to_string(),
                result: r#"{"temperature": 18, "conditions": "partly cloudy"}"#.to_string(),
                timestamp: now,
            }),
            // LLM final response
            ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
                iteration: 2,
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "The weather in Tokyo is 18°C with partly cloudy skies.".to_string(),
                }],
                timestamp: now,
            }),
            ThreadEvent::AgentCompleted(AgentCompletedData {
                total_iterations: 2,
                final_response: "The weather in Tokyo is 18°C with partly cloudy skies."
                    .to_string(),
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        // Should have 4 messages: user, assistant (with tool), tool result, assistant (final)
        assert_eq!(messages.len(), 4);

        // User message
        assert_eq!(messages[0].role, MessageRole::User);

        // Assistant message with text + tool use
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].content.len(), 2);
        match &messages[1].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Let me check the weather for you."),
            _ => panic!("Expected text content"),
        }
        match &messages[1].content[1] {
            ContentBlock::ToolCall {
                tool_call_id,
                name,
                input,
            } => {
                assert_eq!(tool_call_id, "toolu_123");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "Tokyo");
            }
            _ => panic!("Expected tool call content"),
        }

        // Tool result message
        assert_eq!(messages[2].role, MessageRole::Tool);
        assert_eq!(messages[2].content.len(), 1);
        match &messages[2].content[0] {
            ContentBlock::ToolResult {
                tool_call_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_call_id, "toolu_123");
                assert!(content.contains("18"));
                assert!(!is_error);
            }
            _ => panic!("Expected tool result content"),
        }

        // Final assistant message
        assert_eq!(messages[3].role, MessageRole::Assistant);
        match &messages[3].content[0] {
            ContentBlock::Text { text } => assert!(text.contains("18°C")),
            _ => panic!("Expected text content"),
        }
    }

    // =============================================================================
    // Multi-Turn Tool Use Tests
    // =============================================================================

    #[test]
    fn test_multi_turn_tool_use() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Calculate 5+3 and then multiply by 2".to_string(),
                timestamp: now,
            }),
            // First tool use: addition
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "tool_use".to_string(),
                content_blocks: vec![ContentBlockData::ToolUse {
                    id: "toolu_1".to_string(),
                    name: "add".to_string(),
                    input: json!({"a": 5, "b": 3}),
                }],
                timestamp: now,
            }),
            ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
                tool_use_id: "toolu_1".to_string(),
                name: "add".to_string(),
                result: "8".to_string(),
                timestamp: now,
            }),
            // Second tool use: multiplication
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "tool_use".to_string(),
                content_blocks: vec![ContentBlockData::ToolUse {
                    id: "toolu_2".to_string(),
                    name: "multiply".to_string(),
                    input: json!({"a": 8, "b": 2}),
                }],
                timestamp: now,
            }),
            ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
                tool_use_id: "toolu_2".to_string(),
                name: "multiply".to_string(),
                result: "16".to_string(),
                timestamp: now,
            }),
            // Final response
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "The result is 16.".to_string(),
                }],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        // Should have 6 messages: user, asst+tool, tool_result, asst+tool, tool_result, asst
        assert_eq!(messages.len(), 6);

        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[2].role, MessageRole::Tool);
        assert_eq!(messages[3].role, MessageRole::Assistant);
        assert_eq!(messages[4].role, MessageRole::Tool);
        assert_eq!(messages[5].role, MessageRole::Assistant);
    }

    // =============================================================================
    // Failed Tool Execution Tests
    // =============================================================================

    #[test]
    fn test_failed_tool_execution() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Get the weather".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "tool_use".to_string(),
                content_blocks: vec![ContentBlockData::ToolUse {
                    id: "toolu_fail".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "InvalidPlace"}),
                }],
                timestamp: now,
            }),
            ThreadEvent::ToolExecutionFailed(ToolExecutionFailedData {
                tool_use_id: "toolu_fail".to_string(),
                name: "get_weather".to_string(),
                error: "Location not found".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "I couldn't find that location.".to_string(),
                }],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        assert_eq!(messages.len(), 4);

        // Tool error message
        assert_eq!(messages[2].role, MessageRole::Tool);
        match &messages[2].content[0] {
            ContentBlock::ToolResult {
                tool_call_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_call_id, "toolu_fail");
                assert_eq!(content, "Location not found");
                assert!(is_error); // Key check: error flag is set
            }
            _ => panic!("Expected tool result content"),
        }
    }

    // =============================================================================
    // Mixed Content Tests
    // =============================================================================

    #[test]
    fn test_mixed_text_and_tool_use() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "What's 2+2 and the weather?".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "tool_use".to_string(),
                content_blocks: vec![
                    ContentBlockData::Text {
                        text: "Let me calculate and check the weather.".to_string(),
                    },
                    ContentBlockData::ToolUse {
                        id: "toolu_1".to_string(),
                        name: "calculate".to_string(),
                        input: json!({"expr": "2+2"}),
                    },
                    ContentBlockData::ToolUse {
                        id: "toolu_2".to_string(),
                        name: "get_weather".to_string(),
                        input: json!({"location": "here"}),
                    },
                ],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        assert_eq!(messages.len(), 2);

        // Assistant message should have text + 2 tool uses
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].content.len(), 3);

        match &messages[1].content[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "Let me calculate and check the weather.")
            }
            _ => panic!("Expected text first"),
        }

        match &messages[1].content[1] {
            ContentBlock::ToolCall { name, .. } => assert_eq!(name, "calculate"),
            _ => panic!("Expected tool call"),
        }

        match &messages[1].content[2] {
            ContentBlock::ToolCall { name, .. } => assert_eq!(name, "get_weather"),
            _ => panic!("Expected tool call"),
        }
    }

    // =============================================================================
    // Edge Cases
    // =============================================================================

    #[test]
    fn test_empty_events() {
        let events: Vec<ThreadEvent> = vec![];
        let messages = project_events_to_messages(&events);
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_streaming_deltas_ignored() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Hello".to_string(),
                timestamp: now,
            }),
            // Many streaming deltas (all should be ignored)
            ThreadEvent::LlmContentDelta(LlmContentDeltaData {
                content_block_index: 0,
                delta_type: "text".to_string(),
                text: "H".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmContentDelta(LlmContentDeltaData {
                content_block_index: 0,
                delta_type: "text".to_string(),
                text: "i".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmToolUseDelta(LlmToolUseDeltaData {
                tool_use_id: "toolu_1".to_string(),
                partial_json: r#"{"lo"#.to_string(),
                timestamp: now,
            }),
            // Only the completed event matters
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "Hi there!".to_string(),
                }],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        // Deltas should be completely ignored
        assert_eq!(messages.len(), 2);
        match &messages[1].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hi there!"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_assistant_message_with_only_tool_uses() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Execute tool".to_string(),
                timestamp: now,
            }),
            // Assistant message with NO text, only tool use
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "tool_use".to_string(),
                content_blocks: vec![ContentBlockData::ToolUse {
                    id: "toolu_1".to_string(),
                    name: "execute".to_string(),
                    input: json!({}),
                }],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].content.len(), 1); // Only tool use, no text
        match &messages[1].content[0] {
            ContentBlock::ToolCall { .. } => {} // OK
            _ => panic!("Expected tool use only"),
        }
    }

    #[test]
    fn test_internal_events_ignored() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Test".to_string(),
                timestamp: now,
            }),
            // Internal events that should be ignored
            ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
                iteration: 1,
                timestamp: now,
            }),
            ThreadEvent::LlmCallStarted(LlmCallStartedData {
                provider: "claude".to_string(),
                model: "sonnet".to_string(),
                message_count: 1,
                timestamp: now,
            }),
            ThreadEvent::LlmToolUseStarted(LlmToolUseStartedData {
                tool_use_id: "toolu_1".to_string(),
                content_block_index: 0,
                name: "test".to_string(),
                timestamp: now,
            }),
            ThreadEvent::ToolExecutionStarted(ToolExecutionStartedData {
                tool_use_id: "toolu_1".to_string(),
                name: "test".to_string(),
                input: json!({}),
                timestamp: now,
            }),
            ThreadEvent::AgentIterationCompleted(AgentIterationCompletedData {
                iteration: 1,
                has_tool_uses: true,
                timestamp: now,
            }),
            ThreadEvent::AgentCompleted(AgentCompletedData {
                total_iterations: 1,
                final_response: "Done".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "Response".to_string(),
                }],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        // Only user message and assistant response should appear
        assert_eq!(messages.len(), 2);
    }

    // =============================================================================
    // Idempotency Tests
    // =============================================================================

    #[test]
    fn test_projection_is_idempotent() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Test".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "Response".to_string(),
                }],
                timestamp: now,
            }),
        ];

        // Project multiple times
        let messages1 = project_events_to_messages(&events);
        let messages2 = project_events_to_messages(&events);
        let messages3 = project_events_to_messages(&events);

        // All should be identical
        assert_eq!(messages1.len(), messages2.len());
        assert_eq!(messages2.len(), messages3.len());

        for i in 0..messages1.len() {
            assert_eq!(messages1[i].role, messages2[i].role);
            assert_eq!(messages2[i].role, messages3[i].role);
            assert_eq!(messages1[i].content.len(), messages2[i].content.len());
        }
    }

    #[test]
    fn test_message_ordering_preserved() {
        let now = Utc::now();
        let events = vec![
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "First".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "Second".to_string(),
                }],
                timestamp: now,
            }),
            ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: "Third".to_string(),
                timestamp: now,
            }),
            ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                stop_reason: "stop".to_string(),
                content_blocks: vec![ContentBlockData::Text {
                    text: "Fourth".to_string(),
                }],
                timestamp: now,
            }),
        ];

        let messages = project_events_to_messages(&events);

        assert_eq!(messages.len(), 4);

        // Verify order
        match &messages[0].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "First"),
            _ => panic!(),
        }
        match &messages[1].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Second"),
            _ => panic!(),
        }
        match &messages[2].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Third"),
            _ => panic!(),
        }
        match &messages[3].content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Fourth"),
            _ => panic!(),
        }
    }
}
