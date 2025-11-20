// GET /threads/{threadId} handler

use crate::models::{Message, MessageContent, MessageType, ThreadResponse};
use chrono::Utc;
use std::convert::Infallible;
use uuid::Uuid;
use warp::http::StatusCode;

pub async fn get_thread_handler(thread_id: Uuid) -> Result<impl warp::Reply, Infallible> {
    println!("GET /threads/{}", thread_id);

    // Create hardcoded messages with various types
    let now = Utc::now();

    let messages = vec![
        Message {
            id: "msg_1".to_string(),
            message_type: MessageType::User,
            timestamp: now - chrono::Duration::minutes(10),
            content: MessageContent::User {
                text: "What's the weather like in San Francisco?".to_string(),
            },
        },
        Message {
            id: "msg_2".to_string(),
            message_type: MessageType::Agent,
            timestamp: now - chrono::Duration::minutes(9),
            content: MessageContent::Agent {
                text: "Let me check the weather for you.".to_string(),
            },
        },
        Message {
            id: "msg_3".to_string(),
            message_type: MessageType::ToolCall,
            timestamp: now - chrono::Duration::minutes(9),
            content: MessageContent::ToolCall {
                tool_name: "get_weather".to_string(),
                arguments: serde_json::json!({
                    "city": "San Francisco",
                    "country": "US"
                }),
            },
        },
        Message {
            id: "msg_4".to_string(),
            message_type: MessageType::ToolResponse,
            timestamp: now - chrono::Duration::minutes(8),
            content: MessageContent::ToolResponse {
                tool_call_id: "msg_3".to_string(),
                result: serde_json::json!({
                    "temperature": 62,
                    "conditions": "Partly cloudy",
                    "humidity": 70
                }),
            },
        },
        Message {
            id: "msg_5".to_string(),
            message_type: MessageType::Agent,
            timestamp: now - chrono::Duration::minutes(8),
            content: MessageContent::Agent {
                text: "The weather in San Francisco is currently 62°F with partly cloudy skies and 70% humidity.".to_string(),
            },
        },
    ];

    let response = ThreadResponse {
        thread_id,
        messages,
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}
