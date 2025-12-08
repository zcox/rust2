//! Event types for event-sourced agent
//!
//! This module defines two types of events:
//! - `ThreadEvent`: Events stored in MessageDB (with full metadata)
//! - `AgentEvent`: Events emitted to callers via the stream (simplified)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::message_db::types::WriteMessage;

// =============================================================================
// ThreadEvent Data Structs (stored in MessageDB)
// =============================================================================

/// User message received
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserMessageReceivedData {
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

/// Agent iteration started
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentIterationStartedData {
    pub iteration: usize,
    pub timestamp: DateTime<Utc>,
}

/// LLM call started
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmCallStartedData {
    pub provider: String,
    pub model: String,
    pub message_count: usize,
    pub timestamp: DateTime<Utc>,
}

/// LLM content delta (streaming text)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmContentDeltaData {
    pub content_block_index: usize,
    pub delta_type: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

/// LLM tool use started
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmToolUseStartedData {
    pub tool_use_id: String,
    pub content_block_index: usize,
    pub name: String,
    pub timestamp: DateTime<Utc>,
}

/// LLM tool use delta (streaming JSON input)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmToolUseDeltaData {
    pub tool_use_id: String,
    pub partial_json: String,
    pub timestamp: DateTime<Utc>,
}

/// LLM tool use completed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmToolUseCompletedData {
    pub tool_use_id: String,
    pub name: String,
    pub input: Value,
    pub timestamp: DateTime<Utc>,
}

/// LLM response completed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmResponseCompletedData {
    pub stop_reason: String,
    pub content_blocks: Vec<ContentBlockData>,
    pub timestamp: DateTime<Utc>,
}

/// Content block data for serialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlockData {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

/// Tool execution started
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolExecutionStartedData {
    pub tool_use_id: String,
    pub name: String,
    pub input: Value,
    pub timestamp: DateTime<Utc>,
}

/// Tool execution completed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolExecutionCompletedData {
    pub tool_use_id: String,
    pub name: String,
    pub result: String,
    pub timestamp: DateTime<Utc>,
}

/// Tool execution failed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolExecutionFailedData {
    pub tool_use_id: String,
    pub name: String,
    pub error: String,
    pub timestamp: DateTime<Utc>,
}

/// Agent iteration completed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentIterationCompletedData {
    pub iteration: usize,
    pub has_tool_uses: bool,
    pub timestamp: DateTime<Utc>,
}

/// Agent completed successfully
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentCompletedData {
    pub total_iterations: usize,
    pub final_response: String,
    pub timestamp: DateTime<Utc>,
}

/// Agent failed with error
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFailedData {
    pub error: String,
    pub details: String,
    pub iteration: usize,
    pub timestamp: DateTime<Utc>,
}

// =============================================================================
// ThreadEvent Enum (all events stored in MessageDB)
// =============================================================================

/// Events stored in the thread stream
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ThreadEvent {
    UserMessageReceived(UserMessageReceivedData),
    AgentIterationStarted(AgentIterationStartedData),
    LlmCallStarted(LlmCallStartedData),
    LlmContentDelta(LlmContentDeltaData),
    LlmToolUseStarted(LlmToolUseStartedData),
    LlmToolUseDelta(LlmToolUseDeltaData),
    LlmToolUseCompleted(LlmToolUseCompletedData),
    LlmResponseCompleted(LlmResponseCompletedData),
    ToolExecutionStarted(ToolExecutionStartedData),
    ToolExecutionCompleted(ToolExecutionCompletedData),
    ToolExecutionFailed(ToolExecutionFailedData),
    AgentIterationCompleted(AgentIterationCompletedData),
    AgentCompleted(AgentCompletedData),
    AgentFailed(AgentFailedData),
}

impl ThreadEvent {
    /// Get the event type name (for MessageDB message_type field)
    pub fn event_type(&self) -> &str {
        match self {
            ThreadEvent::UserMessageReceived(_) => "UserMessageReceived",
            ThreadEvent::AgentIterationStarted(_) => "AgentIterationStarted",
            ThreadEvent::LlmCallStarted(_) => "LlmCallStarted",
            ThreadEvent::LlmContentDelta(_) => "LlmContentDelta",
            ThreadEvent::LlmToolUseStarted(_) => "LlmToolUseStarted",
            ThreadEvent::LlmToolUseDelta(_) => "LlmToolUseDelta",
            ThreadEvent::LlmToolUseCompleted(_) => "LlmToolUseCompleted",
            ThreadEvent::LlmResponseCompleted(_) => "LlmResponseCompleted",
            ThreadEvent::ToolExecutionStarted(_) => "ToolExecutionStarted",
            ThreadEvent::ToolExecutionCompleted(_) => "ToolExecutionCompleted",
            ThreadEvent::ToolExecutionFailed(_) => "ToolExecutionFailed",
            ThreadEvent::AgentIterationCompleted(_) => "AgentIterationCompleted",
            ThreadEvent::AgentCompleted(_) => "AgentCompleted",
            ThreadEvent::AgentFailed(_) => "AgentFailed",
        }
    }

    /// Get the timestamp from the event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ThreadEvent::UserMessageReceived(d) => d.timestamp,
            ThreadEvent::AgentIterationStarted(d) => d.timestamp,
            ThreadEvent::LlmCallStarted(d) => d.timestamp,
            ThreadEvent::LlmContentDelta(d) => d.timestamp,
            ThreadEvent::LlmToolUseStarted(d) => d.timestamp,
            ThreadEvent::LlmToolUseDelta(d) => d.timestamp,
            ThreadEvent::LlmToolUseCompleted(d) => d.timestamp,
            ThreadEvent::LlmResponseCompleted(d) => d.timestamp,
            ThreadEvent::ToolExecutionStarted(d) => d.timestamp,
            ThreadEvent::ToolExecutionCompleted(d) => d.timestamp,
            ThreadEvent::ToolExecutionFailed(d) => d.timestamp,
            ThreadEvent::AgentIterationCompleted(d) => d.timestamp,
            ThreadEvent::AgentCompleted(d) => d.timestamp,
            ThreadEvent::AgentFailed(d) => d.timestamp,
        }
    }
}

// =============================================================================
// Conversion: ThreadEvent -> WriteMessage (for MessageDB storage)
// =============================================================================

impl ThreadEvent {
    /// Convert to WriteMessage for a specific thread stream
    ///
    /// Note: stream_name should be formatted as "thread:v0-{threadId}"
    /// This is a helper that requires the caller to provide stream_name and optional metadata
    pub fn to_write_message(
        &self,
        stream_name: impl Into<String>,
        metadata: Option<Value>,
    ) -> WriteMessage {
        let data = match self {
            ThreadEvent::UserMessageReceived(d) => serde_json::to_value(d),
            ThreadEvent::AgentIterationStarted(d) => serde_json::to_value(d),
            ThreadEvent::LlmCallStarted(d) => serde_json::to_value(d),
            ThreadEvent::LlmContentDelta(d) => serde_json::to_value(d),
            ThreadEvent::LlmToolUseStarted(d) => serde_json::to_value(d),
            ThreadEvent::LlmToolUseDelta(d) => serde_json::to_value(d),
            ThreadEvent::LlmToolUseCompleted(d) => serde_json::to_value(d),
            ThreadEvent::LlmResponseCompleted(d) => serde_json::to_value(d),
            ThreadEvent::ToolExecutionStarted(d) => serde_json::to_value(d),
            ThreadEvent::ToolExecutionCompleted(d) => serde_json::to_value(d),
            ThreadEvent::ToolExecutionFailed(d) => serde_json::to_value(d),
            ThreadEvent::AgentIterationCompleted(d) => serde_json::to_value(d),
            ThreadEvent::AgentCompleted(d) => serde_json::to_value(d),
            ThreadEvent::AgentFailed(d) => serde_json::to_value(d),
        }
        .expect("Failed to serialize ThreadEvent data");

        WriteMessage::new(Uuid::new_v4(), stream_name, self.event_type())
            .with_data(data)
            .with_metadata(metadata.unwrap_or_else(|| serde_json::json!({})))
    }
}

// =============================================================================
// AgentEvent (events streamed to callers)
// =============================================================================

/// Events emitted from the agent stream to callers
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// User message received
    UserMessage(String),

    /// Text content delta
    TextDelta(String),

    /// Tool execution started
    ToolExecutionStarted {
        tool_use_id: String,
        name: String,
        input: Value,
    },

    /// Tool execution completed
    ToolExecutionCompleted {
        tool_use_id: String,
        name: String,
        result: String,
    },

    /// Tool execution failed
    ToolExecutionFailed {
        tool_use_id: String,
        name: String,
        error: String,
    },

    /// Agent iteration started
    IterationStarted { iteration: usize },

    /// Agent completed successfully
    Completed,
}

// =============================================================================
// Conversion: ThreadEvent -> AgentEvent (for streaming)
// =============================================================================

impl TryFrom<ThreadEvent> for AgentEvent {
    type Error = &'static str;

    fn try_from(event: ThreadEvent) -> Result<Self, Self::Error> {
        match event {
            ThreadEvent::UserMessageReceived(data) => Ok(AgentEvent::UserMessage(data.message)),
            ThreadEvent::LlmContentDelta(data) => Ok(AgentEvent::TextDelta(data.text)),
            ThreadEvent::ToolExecutionStarted(data) => Ok(AgentEvent::ToolExecutionStarted {
                tool_use_id: data.tool_use_id,
                name: data.name,
                input: data.input,
            }),
            ThreadEvent::ToolExecutionCompleted(data) => Ok(AgentEvent::ToolExecutionCompleted {
                tool_use_id: data.tool_use_id,
                name: data.name,
                result: data.result,
            }),
            ThreadEvent::ToolExecutionFailed(data) => Ok(AgentEvent::ToolExecutionFailed {
                tool_use_id: data.tool_use_id,
                name: data.name,
                error: data.error,
            }),
            ThreadEvent::AgentIterationStarted(data) => Ok(AgentEvent::IterationStarted {
                iteration: data.iteration,
            }),
            ThreadEvent::AgentCompleted(_) => Ok(AgentEvent::Completed),
            // These events are internal and should not be streamed
            ThreadEvent::LlmCallStarted(_)
            | ThreadEvent::LlmToolUseStarted(_)
            | ThreadEvent::LlmToolUseDelta(_)
            | ThreadEvent::LlmToolUseCompleted(_)
            | ThreadEvent::LlmResponseCompleted(_)
            | ThreadEvent::AgentIterationCompleted(_)
            | ThreadEvent::AgentFailed(_) => Err("Event type is not streamable"),
        }
    }
}

// =============================================================================
// Event Metadata Helpers
// =============================================================================

/// Helper to create event metadata with common fields
pub fn create_event_metadata(
    correlation_id: Option<String>,
    causation_id: Option<String>,
    user_id: Option<String>,
) -> Value {
    let mut metadata = serde_json::Map::new();

    if let Some(cid) = correlation_id {
        metadata.insert("correlation_id".to_string(), Value::String(cid));
    }

    if let Some(cid) = causation_id {
        metadata.insert("causation_id".to_string(), Value::String(cid));
    }

    if let Some(uid) = user_id {
        metadata.insert("user_id".to_string(), Value::String(uid));
    }

    Value::Object(metadata)
}

/// Generate a stream name for a thread
pub fn thread_stream_name(thread_id: &str) -> String {
    format!("thread:v0-{}", thread_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // =============================================================================
    // Serialization/Deserialization Tests
    // =============================================================================

    #[test]
    fn test_user_message_received_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Hello, world!".to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "UserMessageReceived");
        assert_eq!(json["message"], "Hello, world!");
        // Timestamp should be present and parseable
        assert!(json["timestamp"].is_string());

        // Round-trip test
        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_agent_iteration_started_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 1,
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "AgentIterationStarted");
        assert_eq!(json["iteration"], 1);

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_llm_call_started_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::LlmCallStarted(LlmCallStartedData {
            provider: "claude".to_string(),
            model: "claude-sonnet-4-5@20250929".to_string(),
            message_count: 5,
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "LlmCallStarted");
        assert_eq!(json["provider"], "claude");
        assert_eq!(json["model"], "claude-sonnet-4-5@20250929");
        assert_eq!(json["message_count"], 5);

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_llm_content_delta_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::LlmContentDelta(LlmContentDeltaData {
            content_block_index: 0,
            delta_type: "text".to_string(),
            text: "The current weather".to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "LlmContentDelta");
        assert_eq!(json["content_block_index"], 0);
        assert_eq!(json["delta_type"], "text");
        assert_eq!(json["text"], "The current weather");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_llm_tool_use_started_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::LlmToolUseStarted(LlmToolUseStartedData {
            tool_use_id: "toolu_01ABC123".to_string(),
            content_block_index: 1,
            name: "get_weather".to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "LlmToolUseStarted");
        assert_eq!(json["tool_use_id"], "toolu_01ABC123");
        assert_eq!(json["name"], "get_weather");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_llm_tool_use_delta_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::LlmToolUseDelta(LlmToolUseDeltaData {
            tool_use_id: "toolu_01ABC123".to_string(),
            partial_json: r#"{"location": "Tokyo""#.to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "LlmToolUseDelta");
        assert_eq!(json["partial_json"], r#"{"location": "Tokyo""#);

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_llm_tool_use_completed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::LlmToolUseCompleted(LlmToolUseCompletedData {
            tool_use_id: "toolu_01ABC123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "Tokyo", "unit": "celsius"}),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "LlmToolUseCompleted");
        assert_eq!(json["input"]["location"], "Tokyo");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_llm_response_completed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
            stop_reason: "tool_use".to_string(),
            content_blocks: vec![
                ContentBlockData::Text {
                    text: "Let me check the weather".to_string(),
                },
                ContentBlockData::ToolUse {
                    id: "toolu_01ABC123".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"location": "Tokyo"}),
                },
            ],
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "LlmResponseCompleted");
        assert_eq!(json["stop_reason"], "tool_use");
        assert_eq!(json["content_blocks"][0]["type"], "text");
        assert_eq!(json["content_blocks"][1]["type"], "tool_use");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_tool_execution_started_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::ToolExecutionStarted(ToolExecutionStartedData {
            tool_use_id: "toolu_01ABC123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "Tokyo", "unit": "celsius"}),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "ToolExecutionStarted");
        assert_eq!(json["name"], "get_weather");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_tool_execution_completed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
            tool_use_id: "toolu_01ABC123".to_string(),
            name: "get_weather".to_string(),
            result: r#"{"temperature": 18, "conditions": "partly cloudy"}"#.to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "ToolExecutionCompleted");
        assert_eq!(
            json["result"],
            r#"{"temperature": 18, "conditions": "partly cloudy"}"#
        );

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_tool_execution_failed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::ToolExecutionFailed(ToolExecutionFailedData {
            tool_use_id: "toolu_01ABC123".to_string(),
            name: "get_weather".to_string(),
            error: "API rate limit exceeded".to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "ToolExecutionFailed");
        assert_eq!(json["error"], "API rate limit exceeded");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_agent_iteration_completed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::AgentIterationCompleted(AgentIterationCompletedData {
            iteration: 1,
            has_tool_uses: true,
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "AgentIterationCompleted");
        assert_eq!(json["iteration"], 1);
        assert_eq!(json["has_tool_uses"], true);

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_agent_completed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: 2,
            final_response: "The weather is sunny".to_string(),
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "AgentCompleted");
        assert_eq!(json["total_iterations"], 2);
        assert_eq!(json["final_response"], "The weather is sunny");

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    #[test]
    fn test_agent_failed_serialization() {
        let now = Utc::now();
        let event = ThreadEvent::AgentFailed(AgentFailedData {
            error: "MaxIterationsReached".to_string(),
            details: "Exceeded maximum of 10 iterations".to_string(),
            iteration: 10,
            timestamp: now,
        });

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "AgentFailed");
        assert_eq!(json["error"], "MaxIterationsReached");
        assert_eq!(json["iteration"], 10);

        let deserialized: ThreadEvent = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, event);
    }

    // =============================================================================
    // ThreadEvent Methods Tests
    // =============================================================================

    #[test]
    fn test_event_type() {
        let now = Utc::now();
        let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "test".to_string(),
            timestamp: now,
        });
        assert_eq!(event.event_type(), "UserMessageReceived");

        let event = ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: 1,
            final_response: "done".to_string(),
            timestamp: now,
        });
        assert_eq!(event.event_type(), "AgentCompleted");
    }

    #[test]
    fn test_event_timestamp() {
        let now = Utc::now();
        let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "test".to_string(),
            timestamp: now,
        });
        assert_eq!(event.timestamp(), now);
    }

    // =============================================================================
    // WriteMessage Conversion Tests
    // =============================================================================

    #[test]
    fn test_to_write_message() {
        let now = Utc::now();
        let event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Hello!".to_string(),
            timestamp: now,
        });

        let write_msg = event.to_write_message("thread:v0-123", None);

        assert_eq!(write_msg.stream_name, "thread:v0-123");
        assert_eq!(write_msg.message_type, "UserMessageReceived");
        assert_eq!(write_msg.data["message"], "Hello!");
        // Timestamp should be present and parseable
        assert!(write_msg.data["timestamp"].is_string());
    }

    #[test]
    fn test_to_write_message_with_metadata() {
        let now = Utc::now();
        let event = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 1,
            timestamp: now,
        });

        let metadata = create_event_metadata(
            Some("corr-123".to_string()),
            None,
            Some("user-456".to_string()),
        );

        let write_msg = event.to_write_message("thread:v0-abc", Some(metadata.clone()));

        assert_eq!(write_msg.message_type, "AgentIterationStarted");
        assert_eq!(
            write_msg.metadata.as_ref().unwrap()["correlation_id"],
            "corr-123"
        );
        assert_eq!(write_msg.metadata.as_ref().unwrap()["user_id"], "user-456");
    }

    // =============================================================================
    // AgentEvent Conversion Tests
    // =============================================================================

    #[test]
    fn test_agent_event_from_user_message() {
        let now = Utc::now();
        let thread_event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
            message: "Test message".to_string(),
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        assert_eq!(
            agent_event,
            AgentEvent::UserMessage("Test message".to_string())
        );
    }

    #[test]
    fn test_agent_event_from_content_delta() {
        let now = Utc::now();
        let thread_event = ThreadEvent::LlmContentDelta(LlmContentDeltaData {
            content_block_index: 0,
            delta_type: "text".to_string(),
            text: "Hello".to_string(),
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        assert_eq!(agent_event, AgentEvent::TextDelta("Hello".to_string()));
    }

    #[test]
    fn test_agent_event_from_tool_execution_started() {
        let now = Utc::now();
        let thread_event = ThreadEvent::ToolExecutionStarted(ToolExecutionStartedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "Tokyo"}),
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        match agent_event {
            AgentEvent::ToolExecutionStarted {
                tool_use_id,
                name,
                input,
            } => {
                assert_eq!(tool_use_id, "toolu_123");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "Tokyo");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_agent_event_from_tool_execution_completed() {
        let now = Utc::now();
        let thread_event = ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            result: r#"{"temp": 18}"#.to_string(),
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        match agent_event {
            AgentEvent::ToolExecutionCompleted {
                tool_use_id,
                name,
                result,
            } => {
                assert_eq!(tool_use_id, "toolu_123");
                assert_eq!(name, "get_weather");
                assert_eq!(result, r#"{"temp": 18}"#);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_agent_event_from_tool_execution_failed() {
        let now = Utc::now();
        let thread_event = ThreadEvent::ToolExecutionFailed(ToolExecutionFailedData {
            tool_use_id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            error: "Network error".to_string(),
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        match agent_event {
            AgentEvent::ToolExecutionFailed {
                tool_use_id,
                name,
                error,
            } => {
                assert_eq!(tool_use_id, "toolu_123");
                assert_eq!(name, "get_weather");
                assert_eq!(error, "Network error");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_agent_event_from_iteration_started() {
        let now = Utc::now();
        let thread_event = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
            iteration: 3,
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        assert_eq!(agent_event, AgentEvent::IterationStarted { iteration: 3 });
    }

    #[test]
    fn test_agent_event_from_completed() {
        let now = Utc::now();
        let thread_event = ThreadEvent::AgentCompleted(AgentCompletedData {
            total_iterations: 2,
            final_response: "Done".to_string(),
            timestamp: now,
        });

        let agent_event: AgentEvent = thread_event.try_into().unwrap();
        assert_eq!(agent_event, AgentEvent::Completed);
    }

    #[test]
    fn test_agent_event_from_non_streamable() {
        let now = Utc::now();
        let thread_event = ThreadEvent::LlmCallStarted(LlmCallStartedData {
            provider: "claude".to_string(),
            model: "sonnet".to_string(),
            message_count: 5,
            timestamp: now,
        });

        let result: Result<AgentEvent, _> = thread_event.try_into();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Event type is not streamable");
    }

    // =============================================================================
    // Helper Function Tests
    // =============================================================================

    #[test]
    fn test_thread_stream_name() {
        let stream_name = thread_stream_name("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(
            stream_name,
            "thread:v0-550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_create_event_metadata() {
        let metadata = create_event_metadata(
            Some("corr-123".to_string()),
            Some("cause-456".to_string()),
            Some("user-789".to_string()),
        );

        assert_eq!(metadata["correlation_id"], "corr-123");
        assert_eq!(metadata["causation_id"], "cause-456");
        assert_eq!(metadata["user_id"], "user-789");
    }

    #[test]
    fn test_create_event_metadata_partial() {
        let metadata = create_event_metadata(Some("corr-123".to_string()), None, None);

        assert_eq!(metadata["correlation_id"], "corr-123");
        assert!(metadata.get("causation_id").is_none());
        assert!(metadata.get("user_id").is_none());
    }
}
