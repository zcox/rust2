// POST /threads/{threadId} handler

use crate::llm::agent::{events::AgentEvent, EventSourcedAgent};
use crate::models::SendMessageRequest;
use crate::sse::{create_agent_text_event, create_done_event, create_tool_call_event, create_tool_response_event};
use futures_util::StreamExt;
use std::sync::Arc;
use uuid::Uuid;
use warp::sse::Event;

/// Handle POST /api/v1/threads/{threadId}
///
/// Processes a user message through the event-sourced agent and streams
/// the response back to the client via Server-Sent Events (SSE).
///
/// # SSE Event Types
///
/// - `agent_text`: Text chunks from the LLM response
/// - `tool_call`: When the agent calls a tool
/// - `tool_response`: Result from a tool execution
/// - `done`: Signals completion of the agent loop
///
/// # Arguments
///
/// * `thread_id` - The UUID of the conversation thread
/// * `request` - The user's message
/// * `agent` - The event-sourced agent instance (injected dependency)
pub async fn send_message_handler(
    thread_id: Uuid,
    request: SendMessageRequest,
    agent: Arc<EventSourcedAgent>,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("POST /threads/{}: {}", thread_id, request.text);

    // Convert UUID to string for thread_id
    let thread_id_str = thread_id.to_string();

    // Run the agent and get the event stream
    let agent_stream = agent
        .run(thread_id_str.clone(), request.text.clone())
        .await
        .map_err(|e| {
            eprintln!("Agent error for thread {}: {:?}", thread_id, e);
            warp::reject::custom(AgentErrorRejection(e.to_string()))
        })?;

    // Convert AgentEvent stream to SSE Event stream
    let sse_stream = agent_stream.map(move |event_result| {
        match event_result {
            Ok(agent_event) => match agent_event {
                // Map text deltas to agent_text SSE events
                AgentEvent::TextDelta(text) => {
                    create_agent_text_event(thread_id.to_string(), text)
                }

                // Map tool execution started to tool_call SSE events
                AgentEvent::ToolExecutionStarted {
                    tool_use_id,
                    name,
                    input,
                } => create_tool_call_event(tool_use_id, name, input),

                // Map tool execution completed to tool_response SSE events
                AgentEvent::ToolExecutionCompleted {
                    tool_use_id,
                    name: _,
                    result,
                } => {
                    // Parse result string as JSON, or use as-is if it fails
                    let result_value = serde_json::from_str(&result).unwrap_or_else(|_| {
                        serde_json::json!({ "result": result })
                    });
                    create_tool_response_event(
                        format!("response-{}", tool_use_id),
                        tool_use_id,
                        result_value,
                    )
                }

                // Map tool execution failed to tool_response SSE events with error
                AgentEvent::ToolExecutionFailed {
                    tool_use_id,
                    name,
                    error,
                } => {
                    let error_value = serde_json::json!({
                        "error": error,
                        "tool": name
                    });
                    create_tool_response_event(
                        format!("error-{}", tool_use_id),
                        tool_use_id,
                        error_value,
                    )
                }

                // Map completion to done SSE event
                AgentEvent::Completed => create_done_event(),

                // Ignore other events (UserMessage, IterationStarted, etc.)
                // These are internal/informational and not needed for SSE streaming
                _ => {
                    // Return empty event that we'll filter out
                    Ok(Event::default().data(""))
                }
            },
            Err(e) => {
                // Stream error - create error event
                eprintln!("Stream error for thread {}: {:?}", thread_id, e);
                let error_data = serde_json::json!({
                    "error": e.to_string()
                });
                Ok(Event::default()
                    .event("error")
                    .data(error_data.to_string()))
            }
        }
    })
    .filter(|result| {
        // Filter out empty events from ignored AgentEvents
        futures::future::ready(match result {
            Ok(_event) => {
                // Check if event has empty data
                // This is a bit of a hack, but warp doesn't expose the data field
                // So we just keep all events and rely on clients ignoring empty ones
                true
            }
            Err(_) => true,
        })
    });

    Ok(warp::sse::reply(
        warp::sse::keep_alive().stream(sse_stream),
    ))
}

/// Custom rejection type for agent errors
#[derive(Debug)]
#[allow(dead_code)]
struct AgentErrorRejection(String);

impl warp::reject::Reject for AgentErrorRejection {}
