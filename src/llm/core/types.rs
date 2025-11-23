//! Core types for the LLM abstraction layer

use serde::{Deserialize, Serialize};

use super::config::GenerationConfig;
use crate::llm::claude::ClaudeModel;
use crate::llm::gemini::GeminiModel;

/// Request to generate content from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Conversation history
    pub messages: Vec<Message>,
    /// Available tools the model can call
    pub tools: Option<Vec<ToolDeclaration>>,
    /// Generation parameters
    pub config: GenerationConfig,
    /// System prompt/instructions
    pub system: Option<String>,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content blocks in the message
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Create a new user message with text content
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: text.into(),
            }],
        }
    }

    /// Create a new assistant message with text content
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text {
                text: text.into(),
            }],
        }
    }

    /// Create a new tool message with a tool result
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error: false,
            }],
        }
    }

    /// Create a new tool message with an error result
    pub fn tool_error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: error.into(),
                is_error: true,
            }],
        }
    }
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Human input
    User,
    /// Model output
    Assistant,
    /// Tool execution result
    Tool,
}

/// Content block within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content
    Text { text: String },
    /// Tool invocation
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool execution result
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// Declaration of a tool available to the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDeclaration {
    /// Function name
    pub name: String,
    /// What the tool does
    pub description: String,
    /// JSON Schema for parameters
    pub input_schema: serde_json::Value,
}

/// Events emitted during streaming generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Response begins
    MessageStart {
        message: MessageMetadata,
    },
    /// New content block begins
    ContentBlockStart {
        index: usize,
        #[serde(rename = "content_block")]
        block: ContentBlockStart,
    },
    /// Incremental content update
    ContentDelta {
        index: usize,
        delta: ContentDelta,
    },
    /// Content block complete
    ContentBlockEnd {
        index: usize,
    },
    /// Message metadata update
    MessageDelta {
        usage: Option<UsageMetadata>,
    },
    /// Response complete
    MessageEnd {
        finish_reason: FinishReason,
        usage: UsageMetadata,
    },
    /// Error occurred
    Error {
        error: String,
    },
}

/// Metadata about a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Message ID
    pub id: String,
    /// Message role
    pub role: MessageRole,
    /// Initial usage metadata (if available)
    pub usage: Option<UsageMetadata>,
}

/// Start of a content block
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockStart {
    /// Text block starting
    Text { text: String },
    /// Tool use block starting
    ToolUse { id: String, name: String },
}

/// Incremental content update
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    /// Text token(s)
    TextDelta { text: String },
    /// Partial tool call data
    ToolUseDelta { partial: PartialToolUse },
}

/// Partial tool use information (accumulating)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialToolUse {
    /// Tool use ID (if available)
    pub id: Option<String>,
    /// Tool name (if available)
    pub name: Option<String>,
    /// Partial JSON input (accumulating)
    pub partial_json: String,
}

/// Reason why generation finished
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural completion
    EndTurn,
    /// Natural completion (alternative name)
    Stop,
    /// Hit token limit
    MaxTokens,
    /// Hit stop sequence
    StopSequence,
    /// Waiting for tool execution
    ToolUse,
    /// Blocked by safety filters
    Safety,
    /// Provider-specific reason
    Other(String),
}

/// Token usage information
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UsageMetadata {
    /// Prompt tokens consumed
    pub input_tokens: u32,
    /// Response tokens generated
    pub output_tokens: u32,
    /// Sum of input and output
    pub total_tokens: u32,
}

impl UsageMetadata {
    /// Create new usage metadata
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        }
    }

    /// Add usage from another metadata
    pub fn add(&mut self, other: &UsageMetadata) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.total_tokens = self.input_tokens + self.output_tokens;
    }
}

/// Unified model enum for all supported LLM providers
#[derive(Debug, Clone)]
pub enum Model {
    /// Anthropic Claude model on Vertex AI
    Claude(ClaudeModel),
    /// Google Gemini model on Vertex AI
    Gemini(GeminiModel),
}

impl Model {
    /// Get the model identifier as a string
    pub fn as_str(&self) -> &str {
        match self {
            Model::Claude(model) => model.as_str(),
            Model::Gemini(model) => model.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_user_constructor() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_message_assistant_constructor() {
        let msg = Message::assistant("Hi there");
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "Hi there"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_message_tool_result_constructor() {
        let msg = Message::tool_result("tool-123", "result data");
        assert_eq!(msg.role, MessageRole::Tool);
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool-123");
                assert_eq!(content, "result data");
                assert!(!is_error);
            }
            _ => panic!("Expected tool result content"),
        }
    }

    #[test]
    fn test_message_tool_error_constructor() {
        let msg = Message::tool_error("tool-456", "error message");
        assert_eq!(msg.role, MessageRole::Tool);
        match &msg.content[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool-456");
                assert_eq!(content, "error message");
                assert!(is_error);
            }
            _ => panic!("Expected tool result content"),
        }
    }

    #[test]
    fn test_usage_metadata_new() {
        let usage = UsageMetadata::new(100, 50);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_usage_metadata_add() {
        let mut usage = UsageMetadata::new(100, 50);
        let other = UsageMetadata::new(20, 30);
        usage.add(&other);
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.output_tokens, 80);
        assert_eq!(usage.total_tokens, 200);
    }

    #[test]
    fn test_content_block_serialization() {
        let text_block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&text_block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello\""));

        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text block"),
        }
    }

    #[test]
    fn test_tool_use_block_serialization() {
        let tool_block = ContentBlock::ToolUse {
            id: "tool-1".to_string(),
            name: "get_weather".to_string(),
            input: serde_json::json!({"location": "SF"}),
        };
        let json = serde_json::to_string(&tool_block).unwrap();
        assert!(json.contains("\"type\":\"tool_use\""));

        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            ContentBlock::ToolUse { id, name, .. } => {
                assert_eq!(id, "tool-1");
                assert_eq!(name, "get_weather");
            }
            _ => panic!("Expected tool use block"),
        }
    }

    #[test]
    fn test_tool_result_serialization() {
        let result_block = ContentBlock::ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "72°F".to_string(),
            is_error: false,
        };
        let json = serde_json::to_string(&result_block).unwrap();
        assert!(json.contains("\"type\":\"tool_result\""));

        let deserialized: ContentBlock = serde_json::from_str(&json).unwrap();
        match deserialized {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool-1");
                assert_eq!(content, "72°F");
                assert!(!is_error);
            }
            _ => panic!("Expected tool result block"),
        }
    }

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let role = MessageRole::Assistant;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"assistant\"");

        let role = MessageRole::Tool;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"tool\"");
    }

    #[test]
    fn test_finish_reason_serialization() {
        let reason = FinishReason::Stop;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, "\"stop\"");

        let reason = FinishReason::MaxTokens;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, "\"max_tokens\"");

        let reason = FinishReason::ToolUse;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, "\"tool_use\"");
    }
}
