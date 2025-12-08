# AI Agent Chat API Specification

## Overview

HTTP+JSON web service providing functionality for an AI agent chat UI. Supports retrieving conversation history and streaming agent responses.

## Base URL

`/api/v1`

## Endpoints

### 1. Get Thread Messages

Retrieve all messages in a conversation thread.

**Endpoint:** `GET /threads/{threadId}`

**Path Parameters:**
- `threadId` (UUID, required): Unique identifier for the conversation thread (UUID v4 format)

**Response:**
- Status: `200 OK`
- Content-Type: `application/json`

**Response Body:**
```json
{
  "threadId": "uuid",
  "messages": [
    {
      "message_id": "string",
      "type": "user" | "agent" | "tool_call" | "tool_result",
      "timestamp": "ISO 8601 string",
      "content": "varies by type"
    }
  ]
}
```

**Message Types:**

1. **User Message**
```json
{
  "message_id": "string",
  "type": "user",
  "timestamp": "2025-11-19T10:30:00Z",
  "content": {
    "text": "string"
  }
}
```

2. **Agent Message**
```json
{
  "message_id": "string",
  "type": "agent",
  "timestamp": "2025-11-19T10:30:01Z",
  "content": {
    "text": "string"
  }
}
```

3. **Tool Call**
```json
{
  "message_id": "string",
  "type": "tool_call",
  "timestamp": "2025-11-19T10:30:02Z",
  "content": {
    "tool_call_id": "string",
    "toolName": "string",
    "arguments": {}
  }
}
```

4. **Tool Result**
```json
{
  "message_id": "string",
  "type": "tool_result",
  "timestamp": "2025-11-19T10:30:03Z",
  "content": {
    "tool_result_id": "string",
    "tool_call_id": "string",
    "result": {}
  }
}
```

### 2. Send Message to Thread

Send a new user message to a thread and stream the agent's response.

**Endpoint:** `POST /threads/{threadId}`

**Path Parameters:**
- `threadId` (UUID, required): Unique identifier for the conversation thread (UUID v4 format)

**Request Headers:**
- Content-Type: `application/json`

**Request Body:**
```json
{
  "text": "string"
}
```

**Response:**
- Status: `200 OK`
- Content-Type: `text/event-stream`

**Response Stream (SSE):**

The server streams events containing chunks of the agent response. A single response may contain multiple agent messages interspersed with tool calls and tool responses. Agent text is streamed as chunks, while tool calls and responses are sent as complete events.

**Event Types:**

1. **Agent Text Chunk**
```
event: agent_text
data: {"thread_id": "string", "message_id": "string", "chunk": "string"}
```

2. **Tool Call**
```
event: tool_call
data: {"tool_call_id": "string", "toolName": "string", "arguments": {}}
```

3. **Tool Result**
```
event: tool_result
data: {"tool_result_id": "string", "tool_call_id": "string", "result": {}}
```

4. **Stream End**
```
event: done
data: {}
```

**Example SSE Stream:**
```
event: agent_text
data: {"thread_id": "550e8400-e29b-41d4-a716-446655440000", "message_id": "msg_1", "chunk": "Let me"}

event: agent_text
data: {"thread_id": "550e8400-e29b-41d4-a716-446655440000", "message_id": "msg_1", "chunk": " look that"}

event: agent_text
data: {"thread_id": "550e8400-e29b-41d4-a716-446655440000", "message_id": "msg_1", "chunk": " up for you"}

event: tool_call
data: {"tool_call_id": "toolu_123", "toolName": "lookup", "arguments": {"id": 123}}

event: tool_result
data: {"tool_result_id": "result-toolu_123", "tool_call_id": "toolu_123", "result": "foo bar"}

event: agent_text
data: {"thread_id": "550e8400-e29b-41d4-a716-446655440000", "message_id": "msg_4", "chunk": "The answer"}

event: agent_text
data: {"thread_id": "550e8400-e29b-41d4-a716-446655440000", "message_id": "msg_4", "chunk": " is"}

event: agent_text
data: {"thread_id": "550e8400-e29b-41d4-a716-446655440000", "message_id": "msg_4", "chunk": " foo bar"}

event: done
data: {}
```

## Error Responses

All endpoints may return the following error responses:

**404 Not Found**
```json
{
  "error": "Thread not found",
  "threadId": "uuid"
}
```

**400 Bad Request**
```json
{
  "error": "Invalid request",
  "details": "string"
}
```

**500 Internal Server Error**
```json
{
  "error": "Internal server error",
  "message": "string"
}
```

## Notes

- Thread IDs should be UUID v4 format
- Message IDs should be unique within a thread
- Message IDs in SSE events match the message IDs that would be returned by the GET endpoint
- All chunks for the same message share the same ID
- Timestamps follow ISO 8601 format
- SSE connections should implement proper timeout and reconnection handling
- Only agent text is streamed as chunks; tool calls and tool results are sent as complete JSON objects
