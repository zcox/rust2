// POST /threads/{threadId} handler

use crate::models::SendMessageRequest;
use crate::sse::{
    create_agent_text_event, create_done_event, create_tool_call_event, create_tool_response_event,
};
use futures_util::stream::StreamExt;
use std::convert::Infallible;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use uuid::Uuid;
use warp::sse::Event;

pub async fn send_message_handler(
    thread_id: Uuid,
    request: SendMessageRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("POST /threads/{}: {}", thread_id, request.text);

    // Create SSE event stream
    let event_stream = create_event_stream();

    Ok(warp::sse::reply(
        warp::sse::keep_alive().stream(event_stream),
    ))
}

fn create_event_stream() -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    // Create an interval that ticks every 500ms
    let interval = interval(Duration::from_millis(500));
    let stream = IntervalStream::new(interval);

    // Define the sequence of events to send
    // Note: All chunks for the same message should use the same ID
    let events = vec![
        EventType::AgentText("msg-1".to_string(), "Hello! ".to_string()),
        EventType::AgentText("msg-1".to_string(), "I received ".to_string()),
        EventType::AgentText("msg-1".to_string(), "your message. ".to_string()),
        EventType::AgentText("msg-1".to_string(), "Let me ".to_string()),
        EventType::AgentText("msg-1".to_string(), "check something ".to_string()),
        EventType::AgentText("msg-1".to_string(), "for you. ".to_string()),
        EventType::ToolCall,
        EventType::ToolResponse,
        EventType::AgentText("msg-2".to_string(), "Based on ".to_string()),
        EventType::AgentText("msg-2".to_string(), "the results, ".to_string()),
        EventType::AgentText("msg-2".to_string(), "here's what ".to_string()),
        EventType::AgentText("msg-2".to_string(), "I found: ".to_string()),
        EventType::AgentText("msg-2".to_string(), "The weather is sunny!".to_string()),
        EventType::Done,
    ];

    // Use enumerate to track which event we're on
    stream
        .take(events.len())
        .enumerate()
        .map(move |(i, _tick)| {
            let event = &events[i];
            match event {
                EventType::AgentText(id, text) => create_agent_text_event(id.clone(), text.clone()),
                EventType::ToolCall => create_tool_call_event(
                    "tool-call-456".to_string(),
                    "weather_lookup".to_string(),
                    serde_json::json!({
                        "location": "San Francisco",
                        "units": "fahrenheit"
                    }),
                ),
                EventType::ToolResponse => create_tool_response_event(
                    "response-789".to_string(),
                    "tool-call-456".to_string(),
                    serde_json::json!({
                        "temperature": 72,
                        "condition": "sunny",
                        "humidity": 65
                    }),
                ),
                EventType::Done => create_done_event(),
            }
        })
}

enum EventType {
    AgentText(String, String), // (id, text)
    ToolCall,
    ToolResponse,
    Done,
}
