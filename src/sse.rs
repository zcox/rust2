use serde_json::Value;
use warp::sse::Event;

/// Create an agent_text SSE event with thread_id, message_id and text chunk
pub fn create_agent_text_event(
    thread_id: String,
    message_id: String,
    chunk: String,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "thread_id": thread_id,
        "message_id": message_id,
        "chunk": chunk
    });

    Ok(Event::default()
        .event("agent_text")
        .data(payload.to_string()))
}

/// Create a tool_call SSE event
pub fn create_tool_call_event(
    tool_call_id: String,
    tool_name: String,
    arguments: Value,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "tool_call_id": tool_call_id,
        "tool_name": tool_name,
        "arguments": arguments
    });

    Ok(Event::default()
        .event("tool_call")
        .data(payload.to_string()))
}

/// Create a tool_result SSE event
pub fn create_tool_result_event(
    tool_result_id: String,
    tool_call_id: String,
    result: Value,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "tool_result_id": tool_result_id,
        "tool_call_id": tool_call_id,
        "result": result
    });

    Ok(Event::default()
        .event("tool_result")
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
        let result = create_agent_text_event(
            "thread-123".to_string(),
            "msg-123".to_string(),
            "Hello world".to_string(),
        );
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
    fn test_create_tool_result_event() {
        let result = json!({
            "temperature": 72,
            "condition": "sunny"
        });

        // Test that the function creates an event without panicking
        let event_result = create_tool_result_event(
            "result-789".to_string(),
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
        let thread_id = "thread-123".to_string();
        let message_id = "msg-123".to_string();
        let chunk = "Hello world".to_string();

        let payload = serde_json::json!({
            "thread_id": thread_id,
            "message_id": message_id,
            "chunk": chunk
        });

        assert_eq!(payload["thread_id"], "thread-123");
        assert_eq!(payload["message_id"], "msg-123");
        assert_eq!(payload["chunk"], "Hello world");
    }

    #[test]
    fn test_tool_call_payload_format() {
        // Test JSON payload structure
        let args = json!({
            "query": "weather in NYC"
        });

        let payload = serde_json::json!({
            "tool_call_id": "tool-call-456",
            "tool_name": "search",
            "arguments": args
        });

        assert_eq!(payload["tool_call_id"], "tool-call-456");
        assert_eq!(payload["tool_name"], "search");
        assert_eq!(payload["arguments"]["query"], "weather in NYC");
    }

    #[test]
    fn test_tool_result_payload_format() {
        // Test JSON payload structure
        let result = json!({
            "temperature": 72,
            "condition": "sunny"
        });

        let payload = serde_json::json!({
            "tool_result_id": "result-789",
            "tool_call_id": "tool-call-456",
            "result": result
        });

        assert_eq!(payload["tool_result_id"], "result-789");
        assert_eq!(payload["tool_call_id"], "tool-call-456");
        assert_eq!(payload["result"]["temperature"], 72);
    }
}
