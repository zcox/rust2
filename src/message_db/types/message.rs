use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Message data for writing to Message DB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteMessage {
    /// Unique identifier for the message
    pub id: Uuid,

    /// Target stream name
    pub stream_name: String,

    /// Message type/class name (e.g., "Withdrawn", "DepositRequested")
    #[serde(rename = "type")]
    pub message_type: String,

    /// Business data payload
    #[serde(default)]
    pub data: Value,

    /// Infrastructural/mechanical data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,

    /// Expected current version for concurrency control
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

impl WriteMessage {
    /// Create a new WriteMessage
    ///
    /// # Example
    ///
    /// ```
    /// use rust2::message_db::types::WriteMessage;
    /// use uuid::Uuid;
    /// use serde_json::json;
    ///
    /// let msg = WriteMessage::new(
    ///     Uuid::new_v4(),
    ///     "account-123",
    ///     "Withdrawn",
    /// )
    /// .with_data(json!({ "amount": 50, "currency": "USD" }))
    /// .with_metadata(json!({ "correlation_id": "xyz-789" }))
    /// .with_expected_version(4);
    /// ```
    pub fn new(id: Uuid, stream_name: impl Into<String>, message_type: impl Into<String>) -> Self {
        Self {
            id,
            stream_name: stream_name.into(),
            message_type: message_type.into(),
            data: Value::Object(serde_json::Map::new()),
            metadata: None,
            expected_version: None,
        }
    }

    /// Set the data payload (builder pattern)
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    /// Set the metadata (builder pattern)
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the expected version for optimistic concurrency control (builder pattern)
    pub fn with_expected_version(mut self, version: i64) -> Self {
        self.expected_version = Some(version);
        self
    }
}

/// Message data read from Message DB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for the message
    pub id: Uuid,

    /// Name of the stream containing the message
    pub stream_name: String,

    /// Message type/class name
    #[serde(rename = "type")]
    pub message_type: String,

    /// Business data payload
    pub data: Value,

    /// Infrastructural/mechanical data
    pub metadata: Option<Value>,

    /// Ordinal position in the stream (0-based)
    pub position: i64,

    /// Ordinal position in entire message store
    pub global_position: i64,

    /// UTC timestamp when message was written
    pub time: DateTime<Utc>,
}

impl Message {
    /// Get the correlation ID from metadata if present
    pub fn correlation_id(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("correlation_id"))
            .and_then(|v| v.as_str())
    }

    /// Get the causation ID from metadata if present
    pub fn causation_id(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("causation_id"))
            .and_then(|v| v.as_str())
    }

    /// Get the reply stream name from metadata if present
    pub fn reply_stream_name(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("reply_stream_name"))
            .and_then(|v| v.as_str())
    }

    /// Get the schema version from metadata if present
    pub fn schema_version(&self) -> Option<&str> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("schema_version"))
            .and_then(|v| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_write_message_builder() {
        let id = Uuid::new_v4();
        let msg = WriteMessage::new(id, "account-123", "Withdrawn")
            .with_data(json!({ "amount": 50 }))
            .with_metadata(json!({ "correlation_id": "xyz" }))
            .with_expected_version(4);

        assert_eq!(msg.id, id);
        assert_eq!(msg.stream_name, "account-123");
        assert_eq!(msg.message_type, "Withdrawn");
        assert_eq!(msg.data["amount"], 50);
        assert_eq!(msg.metadata.as_ref().unwrap()["correlation_id"], "xyz");
        assert_eq!(msg.expected_version, Some(4));
    }

    #[test]
    fn test_message_metadata_helpers() {
        let msg = Message {
            id: Uuid::new_v4(),
            stream_name: "account-123".to_string(),
            message_type: "Withdrawn".to_string(),
            data: json!({}),
            metadata: Some(json!({
                "correlation_id": "corr-123",
                "causation_id": "cause-456",
                "reply_stream_name": "replies-789",
                "schema_version": "2"
            })),
            position: 0,
            global_position: 1,
            time: Utc::now(),
        };

        assert_eq!(msg.correlation_id(), Some("corr-123"));
        assert_eq!(msg.causation_id(), Some("cause-456"));
        assert_eq!(msg.reply_stream_name(), Some("replies-789"));
        assert_eq!(msg.schema_version(), Some("2"));
    }

    #[test]
    fn test_message_metadata_helpers_none() {
        let msg = Message {
            id: Uuid::new_v4(),
            stream_name: "account-123".to_string(),
            message_type: "Withdrawn".to_string(),
            data: json!({}),
            metadata: None,
            position: 0,
            global_position: 1,
            time: Utc::now(),
        };

        assert_eq!(msg.correlation_id(), None);
        assert_eq!(msg.causation_id(), None);
        assert_eq!(msg.reply_stream_name(), None);
        assert_eq!(msg.schema_version(), None);
    }
}
