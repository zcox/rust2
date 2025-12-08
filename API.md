# AI Agent Chat API Documentation

Version: 1.0
Base URL: `http://localhost:3030`

## Overview

This API provides a conversational AI agent interface using event sourcing for state management. The agent supports streaming responses via Server-Sent Events (SSE) and tool execution capabilities.

## Endpoints

### 1. Get Thread History

Retrieves the complete conversation history for a given thread.

**Endpoint:** `GET /api/v1/threads/{threadId}`

**Path Parameters:**
- `threadId` (UUID, required) - The unique identifier for the conversation thread

**Response:**
- Status Code: `200 OK`
- Content-Type: `application/json`

**Response Schema:**

```json
{
  "thread_id": "550e8400-e29b-41d4-a716-446655440000",
  "messages": [
    {
      "message_id": "msg_1",
      "message_type": "user",
      "timestamp": "2024-01-15T10:30:00Z",
      "content": {
        "type": "user",
        "text": "What is the weather today?"
      }
    },
    {
      "message_id": "msg_2",
      "message_type": "agent",
      "timestamp": "2024-01-15T10:30:01Z",
      "content": {
        "type": "agent",
        "text": "I'll check the weather for you."
      }
    },
    {
      "message_id": "msg_3",
      "message_type": "tool_result",
      "timestamp": "2024-01-15T10:30:02Z",
      "content": {
        "type": "tool_result",
        "tool_result_id": "result-toolu_01234",
        "tool_call_id": "toolu_01234",
        "result": {
          "temperature": 72,
          "condition": "sunny"
        }
      }
    }
  ]
}
```

**Example Request:**

```bash
curl http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
```

---

### 2. Send Message (Streaming)

Sends a user message to the agent and streams the response via Server-Sent Events (SSE).

**Endpoint:** `POST /api/v1/threads/{threadId}`

**Path Parameters:**
- `threadId` (UUID, required) - The unique identifier for the conversation thread

**Request Body:**
- Content-Type: `application/json`

```json
{
  "text": "What is the weather today?"
}
```

**Request Schema:**

| Field | Type   | Required | Description           |
|-------|--------|----------|-----------------------|
| text  | string | Yes      | The user's message text |

**Response:**
- Status Code: `200 OK`
- Content-Type: `text/event-stream`

The response is a stream of Server-Sent Events (SSE) with the following event types:

#### SSE Event Types

##### 1. `agent_text`

Streamed text chunks from the agent's response.

```
event: agent_text
data: {"thread_id":"550e8400-e29b-41d4-a716-446655440000","message_id":"msg_1","chunk":"I'll check"}
```

**Data Schema:**
```json
{
  "thread_id": "550e8400-e29b-41d4-a716-446655440000",
  "message_id": "msg_1",
  "chunk": "text fragment"
}
```

##### 2. `tool_call`

Indicates the agent is calling a tool.

```
event: tool_call
data: {"tool_call_id":"toolu_01234","tool_name":"get_weather","arguments":{"location":"NYC"}}
```

**Data Schema:**
```json
{
  "tool_call_id": "toolu_01234",
  "tool_name": "get_weather",
  "arguments": {
    "location": "NYC"
  }
}
```

##### 3. `tool_result`

The result from a tool execution.

```
event: tool_result
data: {"tool_result_id":"result-toolu_01234","tool_call_id":"toolu_01234","result":{"temperature":72,"condition":"sunny"}}
```

**Data Schema:**
```json
{
  "tool_result_id": "result-toolu_01234",
  "tool_call_id": "toolu_01234",
  "result": {
    "temperature": 72,
    "condition": "sunny"
  }
}
```

**Error Response:**
```json
{
  "tool_result_id": "error-toolu_01234",
  "tool_call_id": "toolu_01234",
  "result": {
    "error": "Tool execution failed",
    "tool": "get_weather"
  }
}
```

##### 4. `done`

Signals the completion of the agent's response stream.

```
event: done
data: {}
```

##### 5. `error`

Indicates a stream processing error (not a normal flow event).

```
event: error
data: {"error":"Error description"}
```

**Example Request:**

```bash
curl -X POST http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000 \
  -H "Content-Type: application/json" \
  -d '{"text": "What is the weather today?"}'
```

**Example SSE Stream:**

```
event: agent_text
data: {"thread_id":"550e8400-e29b-41d4-a716-446655440000","message_id":"msg_1","chunk":"I'll "}

event: agent_text
data: {"thread_id":"550e8400-e29b-41d4-a716-446655440000","message_id":"msg_1","chunk":"check "}

event: agent_text
data: {"thread_id":"550e8400-e29b-41d4-a716-446655440000","message_id":"msg_1","chunk":"the weather"}

event: tool_call
data: {"tool_call_id":"toolu_01234","tool_name":"get_weather","arguments":{"location":"current"}}

event: tool_result
data: {"tool_result_id":"result-toolu_01234","tool_call_id":"toolu_01234","result":{"temperature":72,"condition":"sunny"}}

event: agent_text
data: {"thread_id":"550e8400-e29b-41d4-a716-446655440000","message_id":"msg_2","chunk":"It's currently"}

event: agent_text
data: {"thread_id":"550e8400-e29b-41d4-a716-446655440000","message_id":"msg_2","chunk":" 72°F and sunny."}

event: done
data: {}
```

---

## Data Models

### Message

Represents a single message in a conversation thread.

```typescript
interface Message {
  message_id: string;
  message_type: "user" | "agent" | "tool_call" | "tool_result";
  timestamp: string; // ISO 8601 datetime
  content: MessageContent;
}
```

### MessageContent

Tagged union representing different message content types.

```typescript
type MessageContent =
  | { type: "user"; text: string }
  | { type: "agent"; text: string }
  | { type: "tool_call"; tool_call_id: string; tool_name: string; arguments: object }
  | { type: "tool_result"; tool_result_id: string; tool_call_id: string; result: object };
```

### ThreadResponse

Response containing a thread's conversation history.

```typescript
interface ThreadResponse {
  thread_id: string; // UUID
  messages: Message[];
}
```

### SendMessageRequest

Request payload for sending a user message.

```typescript
interface SendMessageRequest {
  text: string;
}
```

---

## Client Implementation Examples

### JavaScript/TypeScript (Fetch API)

```typescript
// Send a message and handle SSE stream
async function sendMessage(threadId: string, text: string) {
  const response = await fetch(
    `http://localhost:3030/api/v1/threads/${threadId}`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text })
    }
  );

  const reader = response.body.getReader();
  const decoder = new TextDecoder();

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    const chunk = decoder.decode(value);
    const lines = chunk.split('\n');

    for (const line of lines) {
      if (line.startsWith('event:')) {
        const eventType = line.substring(7).trim();
        // Handle event type
      } else if (line.startsWith('data:')) {
        const data = JSON.parse(line.substring(6));

        switch (eventType) {
          case 'agent_text':
            console.log('Agent:', data.chunk);
            break;
          case 'tool_call':
            console.log('Calling tool:', data.tool_name);
            break;
          case 'tool_result':
            console.log('Tool result:', data.result);
            break;
          case 'done':
            console.log('Stream complete');
            return;
        }
      }
    }
  }
}

// Get thread history
async function getThread(threadId: string) {
  const response = await fetch(
    `http://localhost:3030/api/v1/threads/${threadId}`
  );
  const thread = await response.json();
  return thread;
}
```

### Python (requests + sseclient)

```python
import requests
import json
from sseclient import SSEClient

# Send message with SSE streaming
def send_message(thread_id: str, text: str):
    url = f"http://localhost:3030/api/v1/threads/{thread_id}"
    response = requests.post(
        url,
        json={"text": text},
        stream=True,
        headers={'Accept': 'text/event-stream'}
    )

    client = SSEClient(response)
    for event in client.events():
        data = json.loads(event.data)

        if event.event == 'agent_text':
            print(f"Agent: {data['chunk']}", end='', flush=True)
        elif event.event == 'tool_call':
            print(f"\nCalling tool: {data['tool_name']}")
        elif event.event == 'tool_result':
            print(f"Tool result: {data['result']}")
        elif event.event == 'done':
            print("\nStream complete")
            break

# Get thread history
def get_thread(thread_id: str):
    url = f"http://localhost:3030/api/v1/threads/{thread_id}"
    response = requests.get(url)
    return response.json()
```

### Rust (reqwest)

```rust
use futures_util::stream::StreamExt;
use reqwest::Client;
use serde_json::Value;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let thread_id = Uuid::new_v4();

    // Send message with SSE streaming
    let response = client
        .post(format!("http://localhost:3030/api/v1/threads/{}", thread_id))
        .json(&serde_json::json!({
            "text": "What is the weather today?"
        }))
        .send()
        .await?;

    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("event:") {
                let event_type = &line[7..].trim();
                println!("Event: {}", event_type);
            } else if line.starts_with("data:") {
                let data: Value = serde_json::from_str(&line[6..])?;
                println!("Data: {:?}", data);
            }
        }
    }

    Ok(())
}
```

---

## Error Handling

### HTTP Status Codes

- `200 OK` - Successful request
- `400 Bad Request` - Invalid request body or malformed UUID
- `404 Not Found` - Thread not found (for GET requests)
- `500 Internal Server Error` - Server-side error

### SSE Error Events

Errors during streaming are communicated via `error` events:

```
event: error
data: {"error":"Description of what went wrong"}
```

Clients should handle these error events and implement appropriate retry logic.

---

## Threading Model

- Each conversation is identified by a unique UUID (`thread_id`)
- Thread IDs can be generated client-side or server-side
- All messages and events for a thread are stored in an event-sourced manner
- Thread history is rebuilt by projecting all events when calling GET

---

## Best Practices

1. **Generate UUIDs Client-Side**: Create thread IDs on the client to avoid coordination overhead
2. **Handle Partial Streams**: Implement reconnection logic for interrupted SSE streams
3. **Buffer Text Chunks**: Collect `agent_text` chunks before displaying to reduce UI flicker
4. **Handle Tool Execution**: Display loading indicators during tool calls
5. **Validate UUIDs**: Ensure thread IDs are valid UUIDs before making requests
6. **Keep Connections Alive**: SSE connections use keep-alive; implement proper timeout handling

---

## Changelog

### Version 1.0 (Current)
- Initial API release
- GET thread history endpoint
- POST message with SSE streaming
- Support for tool calling and execution
