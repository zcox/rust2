// GET /threads/{threadId} handler

use crate::llm::agent::{projection::project_events_to_messages, ThreadStore};
use crate::llm::core::types::{ContentBlock, MessageRole};
use crate::models::{Message, MessageContent, MessageType, ThreadResponse};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;
use warp::http::StatusCode;

/// Handle GET /api/v1/threads/{threadId}
///
/// Retrieves the complete conversation history for a thread by:
/// 1. Reading all ThreadEvents from MessageDB
/// 2. Projecting events to LLM Message format
/// 3. Converting to API response format
///
/// # Arguments
///
/// * `thread_id` - The UUID of the conversation thread
/// * `store` - The thread store instance (injected dependency)
pub async fn get_thread_handler(
    thread_id: Uuid,
    store: Arc<ThreadStore>,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("GET /threads/{}", thread_id);

    let thread_id_str = thread_id.to_string();

    // Read all events for this thread from MessageDB
    let events = store
        .read_thread_events(&thread_id_str)
        .await
        .map_err(|e| {
            eprintln!("Failed to read thread events for {}: {:?}", thread_id, e);
            warp::reject::custom(StoreErrorRejection(e.to_string()))
        })?;

    // Project events to LLM messages
    let llm_messages = project_events_to_messages(&events);

    // Convert LLM messages to API message format
    let api_messages: Vec<Message> = llm_messages
        .into_iter()
        .enumerate()
        .map(|(index, llm_msg)| {
            let message_id = format!("msg_{}", index + 1);
            let timestamp = Utc::now(); // TODO: Extract from event timestamps

            match llm_msg.role {
                MessageRole::User => {
                    // Extract text from user message
                    let text = llm_msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");

                    Message {
                        id: message_id,
                        message_type: MessageType::User,
                        timestamp,
                        content: MessageContent::User { text },
                    }
                }
                MessageRole::Assistant => {
                    // For assistant messages, we need to handle both text and tool uses
                    // For now, extract just the text portion
                    let text = llm_msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");

                    // Check if there are tool uses
                    let has_tool_uses = llm_msg
                        .content
                        .iter()
                        .any(|block| matches!(block, ContentBlock::ToolUse { .. }));

                    if has_tool_uses {
                        // If there are tool uses, we'll create multiple messages
                        // For simplicity, just return the text part for now
                        // TODO: Properly expand tool uses into separate ToolCall messages
                        Message {
                            id: message_id,
                            message_type: MessageType::Agent,
                            timestamp,
                            content: MessageContent::Agent {
                                text: if text.is_empty() {
                                    "(calling tools)".to_string()
                                } else {
                                    text
                                },
                            },
                        }
                    } else {
                        Message {
                            id: message_id,
                            message_type: MessageType::Agent,
                            timestamp,
                            content: MessageContent::Agent { text },
                        }
                    }
                }
                MessageRole::Tool => {
                    // Extract tool result
                    if let Some(ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    }) = llm_msg.content.first()
                    {
                        if *is_error {
                            Message {
                                id: message_id,
                                message_type: MessageType::ToolResponse,
                                timestamp,
                                content: MessageContent::ToolResponse {
                                    tool_call_id: tool_use_id.clone(),
                                    result: serde_json::json!({
                                        "error": content
                                    }),
                                },
                            }
                        } else {
                            // Try to parse content as JSON, or wrap as string
                            let result = serde_json::from_str(content).unwrap_or_else(|_| {
                                serde_json::json!({ "result": content })
                            });

                            Message {
                                id: message_id,
                                message_type: MessageType::ToolResponse,
                                timestamp,
                                content: MessageContent::ToolResponse {
                                    tool_call_id: tool_use_id.clone(),
                                    result,
                                },
                            }
                        }
                    } else {
                        // Shouldn't happen, but provide fallback
                        Message {
                            id: message_id,
                            message_type: MessageType::Agent,
                            timestamp,
                            content: MessageContent::Agent {
                                text: "(invalid tool result)".to_string(),
                            },
                        }
                    }
                }
            }
        })
        .collect();

    let response = ThreadResponse {
        thread_id,
        messages: api_messages,
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

/// Custom rejection type for store errors
#[derive(Debug)]
#[allow(dead_code)]
struct StoreErrorRejection(String);

impl warp::reject::Reject for StoreErrorRejection {}
