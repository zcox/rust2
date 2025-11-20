# AI Agent Chat API - Rust Implementation

A Server-Sent Events (SSE) based chat API server built with Rust, Warp, and Tokio.

## Features

- RESTful API endpoints for thread message management
- Server-Sent Events (SSE) for real-time streaming responses
- Support for multiple message types: user messages, agent responses, tool calls, and tool responses
- Asynchronous request handling with Tokio
- JSON serialization with Serde

## Dependencies

- **Warp 0.4**: Web framework for HTTP server and SSE streaming
- **Tokio**: Async runtime
- **Serde**: JSON serialization/deserialization
- **UUID**: Thread ID generation and validation
- **Chrono**: Timestamp handling

## Getting Started

### Building the Project

```bash
# Check code without building
cargo check

# Run linter
cargo clippy

# Format code
cargo fmt

# Build the project
cargo build

# Build optimized release version
cargo build --release
```

### Running the Server

```bash
cargo run
```

The server will start on `http://127.0.0.1:3030`

### Running Tests

```bash
cargo test
```

## API Endpoints

### GET /api/v1/threads/{threadId}

Retrieve all messages for a specific thread.

**Example:**
```bash
curl http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
```

**Response:**
```json
{
  "thread_id": "550e8400-e29b-41d4-a716-446655440000",
  "messages": [
    {
      "id": "msg_1",
      "message_type": "user",
      "timestamp": "2025-11-20T05:14:43.013756Z",
      "content": {
        "type": "user",
        "text": "What's the weather like in San Francisco?"
      }
    },
    {
      "id": "msg_2",
      "message_type": "agent",
      "timestamp": "2025-11-20T05:15:43.013756Z",
      "content": {
        "type": "agent",
        "text": "Let me check the weather for you."
      }
    }
  ]
}
```

### POST /api/v1/threads/{threadId}

Send a message to a thread and receive streaming SSE responses.

**Example:**
```bash
curl -N -H "Content-Type: application/json" \
  -d '{"text":"Hello"}' \
  http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
```

**Request Body:**
```json
{
  "text": "Your message here"
}
```

**SSE Response Stream:**
The server will stream multiple events:

```
event:agent_text
data:{"chunk":"Hello! ","id":"msg-0"}

event:agent_text
data:{"chunk":"I received ","id":"msg-1"}

event:tool_call
data:{"arguments":{"location":"San Francisco","units":"fahrenheit"},"id":"tool-call-456","tool_name":"weather_lookup"}

event:tool_response
data:{"id":"response-789","result":{"condition":"sunny","humidity":65,"temperature":72},"tool_call_id":"tool-call-456"}

event:done
data:{}
```

## SSE Event Types

### agent_text
Text chunks from the agent's response.
```json
{
  "id": "msg-0",
  "chunk": "Hello! "
}
```

### tool_call
Agent invoking a tool.
```json
{
  "id": "tool-call-456",
  "tool_name": "weather_lookup",
  "arguments": {
    "location": "San Francisco",
    "units": "fahrenheit"
  }
}
```

### tool_response
Result from a tool invocation.
```json
{
  "id": "response-789",
  "tool_call_id": "tool-call-456",
  "result": {
    "temperature": 72,
    "condition": "sunny",
    "humidity": 65
  }
}
```

### done
Signals the end of the event stream.
```json
{}
```

## Project Structure

```
src/
├── main.rs              # Entry point, server setup
├── routes.rs            # Route definitions and handlers
├── models.rs            # Data structures (Message, Thread, etc.)
├── handlers/
│   ├── mod.rs
│   ├── get_thread.rs    # GET /threads/{threadId} handler
│   └── send_message.rs  # POST /threads/{threadId} handler
└── sse.rs               # SSE streaming utilities
```

## Development

This implementation currently uses hardcoded responses for demonstration purposes. Future enhancements will include:

- Database integration for message persistence
- LLM provider integration for dynamic responses
- Thread management (create/delete)
- Authentication
- Rate limiting
- Metrics and monitoring

## Testing with Browser

You can also test the SSE endpoint using JavaScript in a browser:

```javascript
const eventSource = new EventSource('http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000');

eventSource.addEventListener('agent_text', (event) => {
  const data = JSON.parse(event.data);
  console.log('Agent text:', data.chunk);
});

eventSource.addEventListener('tool_call', (event) => {
  const data = JSON.parse(event.data);
  console.log('Tool call:', data.tool_name, data.arguments);
});

eventSource.addEventListener('tool_response', (event) => {
  const data = JSON.parse(event.data);
  console.log('Tool response:', data.result);
});

eventSource.addEventListener('done', () => {
  console.log('Stream complete');
  eventSource.close();
});
```

## License

MIT
