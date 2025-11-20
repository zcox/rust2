// Data structures (Message, Thread, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Message Types Enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    User,
    Agent,
    ToolCall,
    ToolResponse,
}

// Message Content Variants
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    User {
        text: String,
    },
    Agent {
        text: String,
    },
    ToolCall {
        tool_name: String,
        arguments: serde_json::Value,
    },
    ToolResponse {
        tool_call_id: String,
        result: serde_json::Value,
    },
}

// Message Struct
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub timestamp: DateTime<Utc>,
    pub content: MessageContent,
}

// Thread Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadResponse {
    pub thread_id: Uuid,
    pub messages: Vec<Message>,
}

// Request Types
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    pub text: String,
}

// SSE Event Types
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct AgentTextChunk {
    pub id: String,
    pub chunk: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallEvent {
    pub id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct ToolResponseEvent {
    pub id: String,
    pub tool_call_id: String,
    pub result: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_type_serialization() {
        let msg_type = MessageType::User;
        let serialized = serde_json::to_string(&msg_type).unwrap();
        assert_eq!(serialized, r#""user""#);

        let msg_type = MessageType::Agent;
        let serialized = serde_json::to_string(&msg_type).unwrap();
        assert_eq!(serialized, r#""agent""#);

        let msg_type = MessageType::ToolCall;
        let serialized = serde_json::to_string(&msg_type).unwrap();
        assert_eq!(serialized, r#""toolcall""#);

        let msg_type = MessageType::ToolResponse;
        let serialized = serde_json::to_string(&msg_type).unwrap();
        assert_eq!(serialized, r#""toolresponse""#);
    }

    #[test]
    fn test_message_type_deserialization() {
        let deserialized: MessageType = serde_json::from_str(r#""user""#).unwrap();
        assert_eq!(deserialized, MessageType::User);

        let deserialized: MessageType = serde_json::from_str(r#""agent""#).unwrap();
        assert_eq!(deserialized, MessageType::Agent);

        let deserialized: MessageType = serde_json::from_str(r#""toolcall""#).unwrap();
        assert_eq!(deserialized, MessageType::ToolCall);

        let deserialized: MessageType = serde_json::from_str(r#""toolresponse""#).unwrap();
        assert_eq!(deserialized, MessageType::ToolResponse);
    }

    #[test]
    fn test_user_content_serialization() {
        let content = MessageContent::User {
            text: "Hello".to_string(),
        };
        let serialized = serde_json::to_string(&content).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["type"], "user");
        assert_eq!(value["text"], "Hello");
    }

    #[test]
    fn test_agent_content_serialization() {
        let content = MessageContent::Agent {
            text: "Hi there".to_string(),
        };
        let serialized = serde_json::to_string(&content).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["type"], "agent");
        assert_eq!(value["text"], "Hi there");
    }

    #[test]
    fn test_tool_call_content_serialization() {
        let content = MessageContent::ToolCall {
            tool_name: "calculator".to_string(),
            arguments: json!({"operation": "add", "a": 1, "b": 2}),
        };
        let serialized = serde_json::to_string(&content).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["type"], "tool_call");
        assert_eq!(value["tool_name"], "calculator");
        assert_eq!(value["arguments"]["operation"], "add");
    }

    #[test]
    fn test_tool_response_content_serialization() {
        let content = MessageContent::ToolResponse {
            tool_call_id: "call_123".to_string(),
            result: json!({"result": 3}),
        };
        let serialized = serde_json::to_string(&content).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["type"], "tool_response");
        assert_eq!(value["tool_call_id"], "call_123");
        assert_eq!(value["result"]["result"], 3);
    }

    #[test]
    fn test_message_serialization() {
        let timestamp = Utc::now();
        let message = Message {
            id: "msg_123".to_string(),
            message_type: MessageType::User,
            timestamp,
            content: MessageContent::User {
                text: "Hello".to_string(),
            },
        };
        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: Message = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, message);
    }

    #[test]
    fn test_thread_response_serialization() {
        let thread_id = Uuid::new_v4();
        let timestamp = Utc::now();
        let messages = vec![Message {
            id: "msg_1".to_string(),
            message_type: MessageType::User,
            timestamp,
            content: MessageContent::User {
                text: "Test".to_string(),
            },
        }];
        let response = ThreadResponse {
            thread_id,
            messages,
        };
        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: ThreadResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.thread_id, thread_id);
        assert_eq!(deserialized.messages.len(), 1);
    }

    #[test]
    fn test_send_message_request_deserialization() {
        let json = r#"{"text":"Hello, world!"}"#;
        let request: SendMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.text, "Hello, world!");
    }

    #[test]
    fn test_agent_text_chunk_serialization() {
        let chunk = AgentTextChunk {
            id: "chunk_1".to_string(),
            chunk: "Hello".to_string(),
        };
        let serialized = serde_json::to_string(&chunk).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["id"], "chunk_1");
        assert_eq!(value["chunk"], "Hello");
    }

    #[test]
    fn test_tool_call_event_serialization() {
        let event = ToolCallEvent {
            id: "call_1".to_string(),
            tool_name: "calculator".to_string(),
            arguments: json!({"a": 1, "b": 2}),
        };
        let serialized = serde_json::to_string(&event).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["id"], "call_1");
        assert_eq!(value["tool_name"], "calculator");
    }

    #[test]
    fn test_tool_response_event_serialization() {
        let event = ToolResponseEvent {
            id: "response_1".to_string(),
            tool_call_id: "call_1".to_string(),
            result: json!({"result": 3}),
        };
        let serialized = serde_json::to_string(&event).unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value["id"], "response_1");
        assert_eq!(value["tool_call_id"], "call_1");
    }
}
