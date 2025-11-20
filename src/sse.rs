use serde_json::Value;
use warp::sse::Event;

/// Create an agent_text SSE event with an ID and text chunk
pub fn create_agent_text_event(
    id: String,
    chunk: String,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "id": id,
        "chunk": chunk
    });

    Ok(Event::default()
        .event("agent_text")
        .data(payload.to_string()))
}

/// Create a tool_call SSE event
pub fn create_tool_call_event(
    id: String,
    tool_name: String,
    arguments: Value,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "id": id,
        "tool_name": tool_name,
        "arguments": arguments
    });

    Ok(Event::default()
        .event("tool_call")
        .data(payload.to_string()))
}

/// Create a tool_response SSE event
pub fn create_tool_response_event(
    id: String,
    tool_call_id: String,
    result: Value,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "id": id,
        "tool_call_id": tool_call_id,
        "result": result
    });

    Ok(Event::default()
        .event("tool_response")
        .data(payload.to_string()))
}

/// Create a done SSE event to signal stream completion
pub fn create_done_event() -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({});

    Ok(Event::default().event("done").data(payload.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_agent_text_event() {
        // Test that the function creates an event without panicking
        let result = create_agent_text_event("msg-123".to_string(), "Hello world".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_tool_call_event() {
        let args = json!({
            "query": "weather in NYC"
        });

        // Test that the function creates an event without panicking
        let result =
            create_tool_call_event("tool-call-456".to_string(), "search".to_string(), args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_tool_response_event() {
        let result = json!({
            "temperature": 72,
            "condition": "sunny"
        });

        // Test that the function creates an event without panicking
        let event_result = create_tool_response_event(
            "response-789".to_string(),
            "tool-call-456".to_string(),
            result,
        );
        assert!(event_result.is_ok());
    }

    #[test]
    fn test_create_done_event() {
        // Test that the function creates an event without panicking
        let result = create_done_event();
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_text_payload_format() {
        // Test JSON payload structure
        let id = "msg-123".to_string();
        let chunk = "Hello world".to_string();

        let payload = serde_json::json!({
            "id": id,
            "chunk": chunk
        });

        assert_eq!(payload["id"], "msg-123");
        assert_eq!(payload["chunk"], "Hello world");
    }

    #[test]
    fn test_tool_call_payload_format() {
        // Test JSON payload structure
        let args = json!({
            "query": "weather in NYC"
        });

        let payload = serde_json::json!({
            "id": "tool-call-456",
            "tool_name": "search",
            "arguments": args
        });

        assert_eq!(payload["id"], "tool-call-456");
        assert_eq!(payload["tool_name"], "search");
        assert_eq!(payload["arguments"]["query"], "weather in NYC");
    }

    #[test]
    fn test_tool_response_payload_format() {
        // Test JSON payload structure
        let result = json!({
            "temperature": 72,
            "condition": "sunny"
        });

        let payload = serde_json::json!({
            "id": "response-789",
            "tool_call_id": "tool-call-456",
            "result": result
        });

        assert_eq!(payload["id"], "response-789");
        assert_eq!(payload["tool_call_id"], "tool-call-456");
        assert_eq!(payload["result"]["temperature"], 72);
    }
}
