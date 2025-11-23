//! Server-Sent Events (SSE) parser for Gemini responses

use bytes::Bytes;
use futures::stream::Stream;
use futures::StreamExt;
use std::pin::Pin;

use crate::llm::core::error::LlmError;

use super::types::GenerateContentResponse;

/// Parse a stream of bytes as Gemini SSE events
///
/// Gemini's SSE format uses `data: <json>` lines. This parser:
/// 1. Reads lines from the byte stream
/// 2. Filters for lines starting with "data: "
/// 3. Extracts and parses the JSON payload
/// 4. Returns a stream of parsed responses
pub fn parse_sse_stream(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
) -> Pin<Box<dyn Stream<Item = Result<GenerateContentResponse, LlmError>> + Send>> {
    // Buffer to accumulate partial lines
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

        // Process complete lines
        let mut events = Vec::new();
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer.drain(..=newline_pos);

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Process data lines
            if let Some(data) = line.strip_prefix("data: ") {
                // Parse the JSON payload
                match serde_json::from_str::<GenerateContentResponse>(data) {
                    Ok(response) => events.push(Ok(response)),
                    Err(e) => {
                        events.push(Err(LlmError::SerializationError(format!(
                            "Failed to parse SSE data: {}. Data: {}",
                            e, data
                        ))));
                    }
                }
            }
            // Ignore other line types (event:, id:, etc.)
        }

        // Return all events found in this chunk
        futures::stream::iter(events)
    });

    Box::pin(event_stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn test_parse_simple_sse() {
        let data = b"data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hello\"}]}}]}\n\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let response = result.unwrap().unwrap();
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(response.candidates[0].content.role, "model");
    }

    #[tokio::test]
    async fn test_parse_multiple_events() {
        let data1 = b"data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hello\"}]}}]}\n";
        let data2 = b"data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\" World\"}]}}]}\n";

        let byte_stream = Box::pin(stream::iter(vec![
            Ok(Bytes::from_static(data1)),
            Ok(Bytes::from_static(data2)),
        ]));

        let mut sse_stream = parse_sse_stream(byte_stream);

        let result1 = sse_stream.next().await;
        assert!(result1.is_some());
        let response1 = result1.unwrap().unwrap();
        match &response1.candidates[0].content.parts[0] {
            super::super::types::Part::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text part"),
        }

        let result2 = sse_stream.next().await;
        assert!(result2.is_some());
        let response2 = result2.unwrap().unwrap();
        match &response2.candidates[0].content.parts[0] {
            super::super::types::Part::Text { text } => assert_eq!(text, " World"),
            _ => panic!("Expected text part"),
        }
    }

    #[tokio::test]
    async fn test_parse_with_empty_lines() {
        let data = b"data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Hello\"}]}}]}\n\n\ndata: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"World\"}]}}]}\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);

        let result1 = sse_stream.next().await;
        assert!(result1.is_some());

        let result2 = sse_stream.next().await;
        assert!(result2.is_some());
    }

    #[tokio::test]
    async fn test_parse_chunked_data() {
        // Simulate data arriving in chunks that split lines
        let chunk1 = b"data: {\"candidates\":[{\"content\":{\"role\":\"mo";
        let chunk2 = b"del\",\"parts\":[{\"text\":\"Hello\"}]}}]}\n";

        let byte_stream = Box::pin(stream::iter(vec![
            Ok(Bytes::from_static(chunk1)),
            Ok(Bytes::from_static(chunk2)),
        ]));

        let mut sse_stream = parse_sse_stream(byte_stream);

        let result = sse_stream.next().await;
        assert!(result.is_some());
        let response = result.unwrap().unwrap();
        assert_eq!(response.candidates[0].content.role, "model");
    }

    #[tokio::test]
    async fn test_parse_invalid_json() {
        let data = b"data: {invalid json}\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[tokio::test]
    async fn test_parse_with_usage_metadata() {
        let data = b"data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Done\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,\"totalTokenCount\":15}}\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let response = result.unwrap().unwrap();
        assert!(response.usage_metadata.is_some());
        let usage = response.usage_metadata.unwrap();
        assert_eq!(usage.prompt_token_count, 10);
        assert_eq!(usage.candidates_token_count, 5);
        assert_eq!(usage.total_token_count, 15);
    }

    #[tokio::test]
    async fn test_parse_function_call() {
        let data = b"data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"functionCall\":{\"name\":\"get_weather\",\"args\":{\"location\":\"SF\"}}}]}}]}\n";
        let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from_static(data))]));

        let mut sse_stream = parse_sse_stream(byte_stream);
        let result = sse_stream.next().await;

        assert!(result.is_some());
        let response = result.unwrap().unwrap();
        match &response.candidates[0].content.parts[0] {
            super::super::types::Part::FunctionCall { function_call } => {
                assert_eq!(function_call.name, "get_weather");
            }
            _ => panic!("Expected function call part"),
        }
    }
}
