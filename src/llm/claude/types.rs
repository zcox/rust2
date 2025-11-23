//! Claude-specific request and response types
//!
//! These types map directly to the Vertex AI Claude API schema.

use serde::{Deserialize, Serialize};

/// Request to stream raw predictions from Claude via Vertex AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRawPredictRequest {
    /// Required API version for Vertex AI Claude
    pub anthropic_version: String,
    /// Maximum number of tokens to generate (required)
    pub max_tokens: u32,
    /// Array of messages in the conversation
    pub messages: Vec<ClaudeMessage>,
    /// System prompt (top-level field)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Available tools for the model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ClaudeTool>>,
    /// Temperature (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p nucleus sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Always true for streaming
    pub stream: bool,
}

/// A single message in the Claude conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    /// Role: "user" or "assistant"
    pub role: String,
    /// Content (can be string or array of content blocks)
    pub content: ClaudeContent,
}

/// Content can be either a simple string or an array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaudeContent {
    /// Simple text content
    Text(String),
    /// Array of content blocks
    Blocks(Vec<ClaudeContentBlock>),
}

/// A content block within a Claude message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeContentBlock {
    /// Text content
    Text { text: String },
    /// Tool use block (model invoking a tool)
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool result block (application providing tool result)
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Tool definition for Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTool {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
}

/// SSE event types from Claude streaming API
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeStreamEvent {
    /// Message streaming started
    MessageStart {
        message: ClaudeMessageData,
    },
    /// Content block started
    ContentBlockStart {
        index: usize,
        content_block: ClaudeContentBlockStart,
    },
    /// Content block delta (incremental update)
    ContentBlockDelta {
        index: usize,
        delta: ClaudeContentDelta,
    },
    /// Content block stopped
    ContentBlockStop {
        index: usize,
    },
    /// Message delta (metadata update)
    MessageDelta {
        delta: ClaudeMessageDeltaData,
        usage: Option<ClaudeUsage>,
    },
    /// Message streaming stopped
    MessageStop,
    /// Ping event (keep-alive)
    Ping,
    /// Error event
    Error {
        error: ClaudeErrorData,
    },
}

/// Message data from message_start event
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeMessageData {
    /// Message ID
    pub id: String,
    /// Message type (always "message")
    #[serde(rename = "type")]
    pub message_type: String,
    /// Message role (always "assistant" for responses)
    pub role: String,
    /// Initial content (usually empty)
    #[serde(default)]
    pub content: Vec<serde_json::Value>,
    /// Model identifier
    pub model: String,
    /// Stop reason (null during streaming)
    pub stop_reason: Option<String>,
    /// Stop sequence that triggered stop (if any)
    pub stop_sequence: Option<String>,
    /// Initial usage metadata
    pub usage: ClaudeUsage,
}

/// Content block start data
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeContentBlockStart {
    /// Text block starting
    Text {
        text: String,
    },
    /// Tool use block starting
    ToolUse {
        id: String,
        name: String,
    },
}

/// Content delta (incremental update)
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeContentDelta {
    /// Text delta
    TextDelta {
        text: String,
    },
    /// Input JSON delta (for tool use arguments)
    InputJsonDelta {
        partial_json: String,
    },
}

/// Message delta data
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeMessageDeltaData {
    /// Stop reason (set when message completes)
    pub stop_reason: Option<String>,
    /// Stop sequence that triggered stop (if any)
    pub stop_sequence: Option<String>,
}

/// Usage metadata
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeUsage {
    /// Input tokens consumed (not present in message_delta updates)
    #[serde(default)]
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
}

/// Error data
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeErrorData {
    /// Error type
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_raw_predict_request_serialization() {
        let request = StreamRawPredictRequest {
            anthropic_version: "vertex-2023-10-16".to_string(),
            max_tokens: 1024,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: ClaudeContent::Text("Hello".to_string()),
            }],
            system: Some("You are helpful".to_string()),
            tools: None,
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            stream: true,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"anthropic_version\":\"vertex-2023-10-16\""));
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"stream\":true"));
    }

    #[test]
    fn test_claude_message_with_text_content() {
        let msg = ClaudeMessage {
            role: "user".to_string(),
            content: ClaudeContent::Text("Hello".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_claude_message_with_blocks() {
        let msg = ClaudeMessage {
            role: "assistant".to_string(),
            content: ClaudeContent::Blocks(vec![
                ClaudeContentBlock::Text {
                    text: "Let me help".to_string(),
                },
                ClaudeContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "get_weather".to_string(),
                    input: serde_json::json!({"location": "SF"}),
                },
            ]),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"type\":\"tool_use\""));
    }

    #[test]
    fn test_claude_content_block_tool_result() {
        let block = ClaudeContentBlock::ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "72°F".to_string(),
            is_error: Some(false),
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"tool_result\""));
        assert!(json.contains("\"tool_use_id\":\"tool-1\""));
        assert!(json.contains("\"content\":\"72°F\""));
    }

    #[test]
    fn test_claude_tool_serialization() {
        let tool = ClaudeTool {
            name: "get_weather".to_string(),
            description: "Get weather for a location".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        };

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"name\":\"get_weather\""));
        assert!(json.contains("\"input_schema\""));
    }

    #[test]
    fn test_claude_stream_event_deserialization() {
        let json = r#"{"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-5","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":0}}}"#;

        let event: ClaudeStreamEvent = serde_json::from_str(json).unwrap();
        match event {
            ClaudeStreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_123");
                assert_eq!(message.role, "assistant");
            }
            _ => panic!("Expected MessageStart event"),
        }
    }

    #[test]
    fn test_content_block_delta_text() {
        let json = r#"{"type":"text_delta","text":"Hello"}"#;
        let delta: ClaudeContentDelta = serde_json::from_str(json).unwrap();

        match delta {
            ClaudeContentDelta::TextDelta { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_content_block_delta_input_json() {
        let json = r#"{"type":"input_json_delta","partial_json":"{\"location\":"}"#;
        let delta: ClaudeContentDelta = serde_json::from_str(json).unwrap();

        match delta {
            ClaudeContentDelta::InputJsonDelta { partial_json } => {
                assert_eq!(partial_json, r#"{"location":"#);
            }
            _ => panic!("Expected InputJsonDelta"),
        }
    }
}
