# Rust2 - AI Agent Chat API + Message DB Client

This repository contains:
1. **AI Agent Chat API**: A Server-Sent Events (SSE) based chat API server built with Rust, Warp, and Tokio
2. **Message DB Client Library**: A high-performance async client for Message DB (PostgreSQL-based event store)

---

## Part 1: AI Agent Chat API

### Features

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

---

## Part 2: Message DB Client Library

### Overview

A high-performance, async Rust client library for [Message DB](https://github.com/message-db/message-db), a PostgreSQL-based event store and message store designed for microservices, event sourcing, and pub/sub architectures.

**Current Status:** Phase 5 (Documentation & Examples) - ✅ Complete

All phases 1-5 are complete! See phase summaries for detailed implementation notes:
- [MESSAGE_DB_PHASE1_SUMMARY.md](MESSAGE_DB_PHASE1_SUMMARY.md) - Foundation
- [MESSAGE_DB_PHASE4_SUMMARY.md](MESSAGE_DB_PHASE4_SUMMARY.md) - Consumer Support

### Features (All Phases Complete)

- ✅ Connection pool management with deadpool-postgres
- ✅ Stream name parsing utilities (category, id, cardinal_id, is_category)
- ✅ Core operations (write_message, get_stream_messages, get_category_messages)
- ✅ Stream queries (get_last_stream_message, stream_version)
- ✅ Full transaction support (begin, commit, rollback)
- ✅ Optimistic concurrency control (expected_version)
- ✅ Consumer pattern with automatic position tracking
- ✅ Consumer groups for horizontal scaling
- ✅ Correlation-based filtering
- ✅ Comprehensive error handling with typed errors
- ✅ Docker-based integration testing
- ✅ Complete API documentation (rustdoc)
- ✅ Extensive examples and guides

### Quick Start

```rust
use rust2::message_db::{MessageDbClient, MessageDbConfig, category, id};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse stream names
    let stream_name = "account:command-123";
    println!("Category: {}", category(stream_name)); // "account:command"
    println!("ID: {:?}", id(stream_name));           // Some("123")

    // Connect to Message DB
    let config = MessageDbConfig::from_connection_string(
        "postgresql://postgres:password@localhost:5433/message_store"
    )?;

    let client = MessageDbClient::new(config).await?;

    Ok(())
}
```

### Running Examples

The library includes comprehensive examples demonstrating all features:

```bash
# Start Message DB
docker-compose up -d

# Foundation and utilities
cargo run --example phase1_demo

# Writing events
cargo run --example writing_events

# Reading streams and categories
cargo run --example reading_streams

# Transactions and atomic operations
cargo run --example transactions

# Optimistic concurrency control
cargo run --example optimistic_concurrency

# Consumer pattern
cargo run --example consumer_example

# Consumer groups for scaling
cargo run --example consumer_groups
```

### Stream Name Parsing Utilities

The library provides utilities for working with Message DB stream names:

```rust
use rust2::message_db::{category, id, cardinal_id, is_category};

// Extract category (with type qualifiers)
category("account-123");                    // "account"
category("account:command-456");            // "account:command"
category("transaction:event+audit-xyz");    // "transaction:event+audit"

// Extract entity ID
id("account-123");                          // Some("123")
id("account:command-456");                  // Some("456")
id("withdrawal:position-consumer-1");       // Some("consumer-1")

// Extract cardinal ID (first segment)
cardinal_id("account-123");                 // Some("123")
cardinal_id("account-123-456");             // Some("123")
cardinal_id("withdrawal:position-consumer-1"); // Some("consumer")

// Check if category (no ID)
is_category("account");                     // true
is_category("account-123");                 // false
is_category("account:command");             // true
```

### Running Tests

```bash
# Run all tests (includes Message DB integration tests)
cargo test

# Run only Message DB tests
cargo test message_db

# Run only integration tests (requires Docker)
cargo test --test integration_test
```

### Message DB Docker Setup

The project includes a `docker-compose.yml` for local development:

```yaml
services:
  messagedb:
    image: ethangarofolo/message-db:1.3.1
    ports:
      - "5433:5432"
    environment:
      POSTGRES_PASSWORD: message_store_password
```

**Connection Details:**
- Host: `localhost`
- Port: `5433` (external) / `5432` (internal)
- Database: `message_store`
- Username: `postgres`
- Password: `message_store_password`

### Examples and Guides

The library includes extensive examples and documentation:

**Examples:**
- `phase1_demo.rs` - Foundation and connection setup
- `writing_events.rs` - Writing events with data and metadata
- `reading_streams.rs` - Reading from streams and categories
- `transactions.rs` - Atomic multi-message writes
- `optimistic_concurrency.rs` - Concurrency control patterns
- `consumer_example.rs` - Consumer pattern with position tracking
- `consumer_groups.rs` - Horizontal scaling with consumer groups

**Guides:**
- [ERROR_HANDLING_GUIDE.md](ERROR_HANDLING_GUIDE.md) - Comprehensive error handling patterns
- [PERFORMANCE_TUNING_GUIDE.md](PERFORMANCE_TUNING_GUIDE.md) - Production performance optimization

### Implementation Status

All planned phases are complete:

- **Phase 1** (✅ Complete): Foundation - connection pool, parsing utilities, testing infrastructure
- **Phase 2** (✅ Complete): Core operations - write_message, read streams/categories
- **Phase 3** (✅ Complete): Transactions - atomic multi-message writes
- **Phase 4** (✅ Complete): Consumer support - polling, position tracking, consumer groups
- **Phase 5** (✅ Complete): Documentation & Examples - guides, examples, API docs

See [RUST_MESSAGE_DB_CLIENT_PLAN.md](RUST_MESSAGE_DB_CLIENT_PLAN.md) for the complete implementation plan.

### Documentation

**Specifications and Plans:**
- [Message DB Client Specification](MESSAGE_DB_CLIENT_SPEC.md) - Complete library specification
- [Technical Plan](RUST_MESSAGE_DB_CLIENT_PLAN.md) - Implementation roadmap and decisions

**Phase Summaries:**
- [Phase 1 Summary](MESSAGE_DB_PHASE1_SUMMARY.md) - Foundation implementation
- [Phase 4 Summary](MESSAGE_DB_PHASE4_SUMMARY.md) - Consumer support implementation

**User Guides:**
- [Error Handling Guide](ERROR_HANDLING_GUIDE.md) - Error types, patterns, and best practices
- [Performance Tuning Guide](PERFORMANCE_TUNING_GUIDE.md) - Optimization and production tuning

**API Documentation:**
```bash
# Generate and view rustdoc documentation
cargo doc --open
```

---

## License

MIT
