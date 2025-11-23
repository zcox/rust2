//! Server-Sent Events (SSE) parser for Claude responses

use bytes::Bytes;
use futures::stream::Stream;
use futures::StreamExt;
use std::pin::Pin;

use crate::llm::core::error::LlmError;

use super::types::ClaudeStreamEvent;

/// Parse a stream of bytes as Claude SSE events
///
/// Claude's SSE format uses:
/// ```
/// event: message_start
/// data: {"type":"message_start",...}
///
/// event: content_block_delta
/// data: {"type":"content_block_delta",...}
/// ```
///
/// This parser:
/// 1. Buffers incoming bytes
/// 2. Scans for event boundaries (double newline)
/// 3. Extracts event type from `event:` line
/// 4. Extracts and parses JSON from `data:` line
/// 5. Returns a stream of parsed events
pub fn parse_sse_stream(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
) -> Pin<Box<dyn Stream<Item = Result<ClaudeStreamEvent, LlmError>> + Send>> {
    // Buffer to accumulate partial events
    let mut buffer = String::new();

    let event_stream = byte_stream.flat_map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(bytes) => bytes,
            Err(e) => {
                return futures::stream::iter(vec![Err(LlmError::StreamError(e.to_string()))]);
            }
        };

        // Convert bytes to string and append to buffer
        let text = match std::str::from_utf8(&chunk) {
            Ok(t) => t,
            Err(e) => {
                return futures::stream::iter(vec![Err(LlmError::StreamError(format!(
                    "Invalid UTF-8 in stream: {}",
                    e
                )))]);
            }
        };

        buffer.push_str(text);

        // Process complete events (delimited by \n\n)
        let mut events = Vec::new();
        while let Some(event_end) = buffer.find("\n\n") {
            let event_text = buffer[..event_end].to_string();
            buffer.drain(..=event_end + 1); // Remove event + one of the newlines

            // Parse the event
            if let Some(parsed_event) = parse_event(&event_text) {
                events.push(parsed_event);
            }
        }

        // Return all events found in this chunk
        futures::stream::iter(events)
    });

    Box::pin(event_stream)
}

/// Parse a single SSE event from its text representation
fn parse_event(event_text: &str) -> Option<Result<ClaudeStreamEvent, LlmError>> {
    let mut event_type: Option<String> = None;
    let mut data: Option<String> = None;

    for line in event_text.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Extract event type
        if let Some(type_val) = line.strip_prefix("event:") {
            event_type = Some(type_val.trim().to_string());
        }

        // Extract data
        if let Some(data_val) = line.strip_prefix("data:") {
            data = Some(data_val.trim().to_string());
        }
    }

    // We need data to parse an event
    let data = data?;

    // Skip ping events (no data)
    if data.is_empty() {
        return None;
    }

    // Parse the JSON data
    match serde_json::from_str::<ClaudeStreamEvent>(&data) {
        Ok(event) => Some(Ok(event)),
        Err(e) => Some(Err(LlmError::SerializationError(format!(
            "Failed to parse Claude SSE event (type: {:?}): {}. Data: {}",
            event_type, e, data
        )))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{ClaudeContentBlockStart, ClaudeContentDelta};
    use futures::stream;

    #[tokio::test]
    async fn test_parse_message_start() {
        let data = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-5\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            ClaudeStreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_123");
                assert_eq!(message.role, "assistant");
                assert_eq!(message.usage.input_tokens, 10);
            }
            _ => panic!("Expected MessageStart event"),
        }
    }

    #[tokio::test]
    async fn test_parse_content_block_start() {
        let data = b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            ClaudeStreamEvent::ContentBlockStart { index, content_block } => {
                assert_eq!(index, 0);
                match content_block {
                    ClaudeContentBlockStart::Text { text } => {
                        assert_eq!(text, "");
                    }
                    _ => panic!("Expected text block"),
                }
            }
            _ => panic!("Expected ContentBlockStart event"),
        }
    }

    #[tokio::test]
    async fn test_parse_content_block_delta() {
        let data = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            ClaudeStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    ClaudeContentDelta::TextDelta { text } => {
                        assert_eq!(text, "Hello");
                    }
                    _ => panic!("Expected text delta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }

    #[tokio::test]
    async fn test_parse_input_json_delta() {
        let data = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"location\\\":\"}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            ClaudeStreamEvent::ContentBlockDelta { delta, .. } => {
                match delta {
                    ClaudeContentDelta::InputJsonDelta { partial_json } => {
                        assert_eq!(partial_json, r#"{"location":"#);
                    }
                    _ => panic!("Expected input json delta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }

    #[tokio::test]
    async fn test_parse_message_delta() {
        let data = b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"input_tokens\":10,\"output_tokens\":25}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            ClaudeStreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason, Some("end_turn".to_string()));
                assert!(usage.is_some());
                let usage = usage.unwrap();
                assert_eq!(usage.output_tokens, 25);
            }
            _ => panic!("Expected MessageDelta event"),
        }
    }

    #[tokio::test]
    async fn test_parse_message_stop() {
        let data = b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            ClaudeStreamEvent::MessageStop => (),
            _ => panic!("Expected MessageStop event"),
        }
    }

    #[tokio::test]
    async fn test_parse_multiple_events() {
        let data = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-5\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);

        let result1 = sse_stream.next().await;
        assert!(result1.is_some());
        match result1.unwrap().unwrap() {
            ClaudeStreamEvent::MessageStart { .. } => (),
            _ => panic!("Expected MessageStart event"),
        }

        let result2 = sse_stream.next().await;
        assert!(result2.is_some());
        match result2.unwrap().unwrap() {
            ClaudeStreamEvent::ContentBlockStart { .. } => (),
            _ => panic!("Expected ContentBlockStart event"),
        }
    }

    #[tokio::test]
    async fn test_parse_chunked_events() {
        // Simulate event arriving in chunks
        let chunk1 = b"event: content_block_delta\ndata: {\"type\":\"content_block";
        let chunk2 = b"_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";

        let byte_stream = Box::pin(stream::iter(vec![
            Ok(Bytes::from_static(chunk1)),
            Ok(Bytes::from_static(chunk2)),
        ]));

        let mut sse_stream = parse_sse_stream(byte_stream);

        let result = sse_stream.next().await;
        assert!(result.is_some());
        match result.unwrap().unwrap() {
            ClaudeStreamEvent::ContentBlockDelta { delta, .. } => {
                match delta {
                    ClaudeContentDelta::TextDelta { text } => {
                        assert_eq!(text, "Hello");
                    }
                    _ => panic!("Expected text delta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }

    #[tokio::test]
    async fn test_parse_ping_event() {
        let data = b"event: ping\ndata: {\"type\":\"ping\"}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        match result.unwrap().unwrap() {
            ClaudeStreamEvent::Ping => (),
            _ => panic!("Expected Ping event"),
        }
    }

    #[tokio::test]
    async fn test_parse_error_event() {
        let data = b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"Invalid API key\"}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        match result.unwrap().unwrap() {
            ClaudeStreamEvent::Error { error } => {
                assert_eq!(error.error_type, "invalid_request_error");
                assert_eq!(error.message, "Invalid API key");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[tokio::test]
    async fn test_parse_tool_use_start() {
        let data = b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool_abc123\",\"name\":\"get_weather\"}}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        match result.unwrap().unwrap() {
            ClaudeStreamEvent::ContentBlockStart { index, content_block } => {
                assert_eq!(index, 1);
                match content_block {
                    ClaudeContentBlockStart::ToolUse { id, name } => {
                        assert_eq!(id, "tool_abc123");
                        assert_eq!(name, "get_weather");
                    }
                    _ => panic!("Expected tool use block"),
                }
            }
            _ => panic!("Expected ContentBlockStart event"),
        }
    }

    #[tokio::test]
    async fn test_parse_invalid_json() {
        let data = b"event: message_delta\ndata: {invalid json}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }
}
