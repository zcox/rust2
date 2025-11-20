# AI Agent Chat API - Rust Implementation Plan

## Project Overview

Implementation of an AI Agent Chat API server in Rust using:
- **Warp 0.4**: Web framework for HTTP server and SSE streaming
- **Serde**: JSON serialization/deserialization
- **Tokio**: Async runtime

This initial version will implement API endpoints with hardcoded responses and logging. Database integration and LLM provider connections will be added in future iterations.

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
warp = { version = "0.4", features = ["server"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
futures-util = "0.3"
tokio-stream = "0.1"
```

## Project Structure

```
src/
├── main.rs           # Entry point, server setup
├── routes.rs         # Route definitions and handlers
├── models.rs         # Data structures (Message, Thread, etc.)
├── handlers/
│   ├── mod.rs
│   ├── get_thread.rs     # GET /threads/{threadId} handler
│   └── send_message.rs   # POST /threads/{threadId} handler
└── sse.rs            # SSE streaming utilities
```

## Data Models (models.rs)

Define structs with Serde derive macros:

1. **Message Types Enum**
   - `MessageType`: User, Agent, ToolCall, ToolResponse

2. **Message Content Variants**
   - `UserContent { text: String }`
   - `AgentContent { text: String }`
   - `ToolCallContent { tool_name: String, arguments: serde_json::Value }`
   - `ToolResponseContent { tool_call_id: String, result: serde_json::Value }`

3. **Message Struct**
   - `id: String`
   - `message_type: MessageType`
   - `timestamp: DateTime<Utc>`
   - `content: MessageContent` (enum)

4. **Thread Response**
   - `thread_id: Uuid`
   - `messages: Vec<Message>`

5. **Request/Response Types**
   - `SendMessageRequest { text: String }`
   - SSE event types: `AgentTextChunk`, `ToolCallEvent`, `ToolResponseEvent`

## Route Configuration (routes.rs)

Set up Warp filters and routes:

1. **Base path filter**: `/api/v1`

2. **GET /threads/{threadId}**
   - Extract UUID from path
   - Combine with handler using `and_then`
   - Handle with `get_thread_handler`

3. **POST /threads/{threadId}**
   - Extract UUID from path
   - Parse JSON body (SendMessageRequest)
   - Handle with `send_message_handler`

4. **Route composition**
   - Combine routes with `.or()`
   - Add CORS if needed
   - Add logging middleware

## Handler: GET Thread Messages (handlers/get_thread.rs)

**Function**: `get_thread_handler(thread_id: Uuid)`

**Logic**:
1. Log the request: `println!("GET /threads/{}", thread_id)`
2. Create hardcoded messages array with various message types:
   - Example user message
   - Example agent message
   - Example tool call
   - Example tool response
3. Build ThreadResponse with thread_id and messages
4. Return JSON response with status 200
5. Handle errors (return 404 if thread not found in future)

**Hardcoded Response Example**:
- 4-5 messages showing complete conversation flow
- Use current timestamp with chrono
- Mix of all message types

## Handler: POST Send Message (handlers/send_message.rs)

**Function**: `send_message_handler(thread_id: Uuid, request: SendMessageRequest)`

**Logic**:
1. Log the request: `println!("POST /threads/{}: {}", thread_id, request.text)`
2. Create SSE event stream
3. Return SSE response using `warp::sse::reply()`

**SSE Stream Implementation**:
1. Use `tokio::time::interval` to simulate streaming delays
2. Create `IntervalStream` wrapper
3. Generate sequence of events:
   - Multiple `agent_text` chunks (split hardcoded response) - **all with same ID** (e.g., "msg-1")
   - One `tool_call` event
   - One `tool_response` event
   - More `agent_text` chunks - **all with different ID** (e.g., "msg-2")
   - Final `done` event
4. Map interval ticks to SSE Events

**Important**: All chunks belonging to the same agent message must use the same `id` field. This allows clients to concatenate chunks with matching IDs to reconstruct complete messages. For example:
- First agent message chunks: all use `id: "msg-1"`
- Second agent message chunks: all use `id: "msg-2"`

**Event Creation** (per Warp API):
- Use `warp::sse::Event::default()`
- Set event type: `.event("agent_text")`, `.event("tool_call")`, etc.
- Set data: `.data(json_string)`
- Return `Result<Event, Infallible>`

## SSE Utilities (sse.rs)

Helper functions for creating SSE events:

1. **`create_agent_text_event(id: String, chunk: String)`**
   - Returns Event with type "agent_text"
   - JSON payload: `{"id": "...", "chunk": "..."}`

2. **`create_tool_call_event(id: String, tool_name: String, arguments: Value)`**
   - Returns Event with type "tool_call"
   - JSON payload per spec

3. **`create_tool_response_event(id: String, tool_call_id: String, result: Value)`**
   - Returns Event with type "tool_response"
   - JSON payload per spec

4. **`create_done_event()`**
   - Returns Event with type "done"
   - Empty JSON payload

## Main Server (main.rs)

**Setup**:
1. Initialize logger (optional: `env_logger` or `pretty_env_logger`)
2. Import route configuration
3. Set up Warp server
4. Bind to `127.0.0.1:3030`
5. Run with `tokio::main` macro

**Code Structure**:
```rust
#[tokio::main]
async fn main() {
    // Optional logging setup
    let routes = configure_routes();

    println!("Starting server on http://127.0.0.1:3030");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}
```

## Error Handling

For this initial version:
1. Use simple Result types with Warp rejections
2. Create custom rejection types for:
   - Thread not found (404)
   - Invalid request (400)
   - Internal error (500)
3. Implement `recover` filter to convert rejections to JSON responses
4. All errors logged to console

## Development Workflow

Use standard Rust tooling throughout development. Run these commands frequently to catch issues early:

**Core Development Commands**:

1. **`cargo check`**
   - Fast compilation check without producing executables
   - Run frequently during development to catch type errors
   - Use after each significant code change

2. **`cargo fmt`**
   - Format code according to Rust style guidelines
   - Run before committing code
   - Ensures consistent code style across the project

3. **`cargo clippy`**
   - Linting tool to catch common mistakes and anti-patterns
   - Fix all warnings before considering code complete
   - May suggest more idiomatic Rust patterns

4. **`cargo build`**
   - Compile the project and produce executable
   - Use `cargo build --release` for optimized production builds
   - Debug builds are faster to compile, use during development

5. **`cargo test`**
   - Run all unit and integration tests
   - Add tests as functionality is implemented
   - All tests must pass before code is considered complete

6. **`cargo run`**
   - Build and run the server in one command
   - Use for local testing and development
   - Server will start on http://127.0.0.1:3030

**Additional Useful Commands**:

7. **`cargo doc --open`**
   - Generate documentation for your project and all dependencies
   - Opens documentation in browser
   - Useful for understanding dependencies and creating internal docs
   - Run periodically to ensure doc comments are correct

8. **`cargo tree`**
   - Display dependency tree
   - Useful for understanding dependency relationships
   - Can help identify duplicate dependencies or bloat

9. **`cargo outdated`** (requires `cargo install cargo-outdated`)
   - Check for outdated dependencies
   - Run periodically to keep dependencies up to date
   - Optional but recommended for maintenance

10. **`cargo audit`** (requires `cargo install cargo-audit`)
    - Audit dependencies for security vulnerabilities
    - Run before releases and periodically during development
    - Checks against RustSec Advisory Database
    - Critical for production applications

11. **`cargo deny`** (requires `cargo install cargo-deny`)
    - More comprehensive dependency auditing
    - Can enforce license policies, ban specific crates, detect duplicate dependencies
    - Optional but recommended for production applications

**Development Cycle**:
1. Write/modify code
2. Run `cargo check` to verify compilation
3. Run `cargo clippy` and fix all warnings
4. Run `cargo fmt` to format code
5. Run `cargo test` to verify tests pass
6. Run `cargo run` to manually test functionality
7. Repeat

**Pre-Release Checklist**:
- Run `cargo audit` to check for security vulnerabilities
- Run `cargo outdated` to identify outdated dependencies (consider updating)
- Run `cargo tree` to verify dependency structure is reasonable
- Generate and review documentation with `cargo doc --open`

**Fix All Errors and Warnings**:
- Zero tolerance for compilation errors
- Fix all `clippy` warnings (use `#[allow(clippy::...)]` only when justified)
- All code must be formatted with `rustfmt`
- No unused imports, variables, or dead code in final implementation

## Testing Strategy

**Automated Testing** (using `cargo test`):
1. Unit tests for data model serialization/deserialization
2. Unit tests for SSE event creation utilities
3. Integration tests for route handlers (future)

**Manual Testing**:
1. Use `curl` for GET endpoint:
   ```bash
   curl http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
   ```

2. Use `curl` for POST with SSE:
   ```bash
   curl -N -H "Content-Type: application/json" \
     -d '{"text":"Hello"}' \
     http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
   ```

3. Browser testing with EventSource API for SSE endpoint

**Validation Points**:
- All `cargo check`, `cargo clippy`, `cargo test` pass without warnings
- Code is formatted with `cargo fmt`
- GET returns valid JSON matching spec
- POST streams SSE events with correct format
- Event types match specification
- JSON payloads deserialize correctly
- Server logs requests

## Implementation Order

For each phase below, run the full development cycle: `cargo check` → `cargo clippy` → `cargo fmt` → `cargo test` → `cargo build` → `cargo run`

1. **Phase 1: Project Setup**
   - Create Cargo.toml with dependencies
   - Set up project structure (directories and empty files)
   - Run `cargo check` to verify dependencies resolve correctly
   - Create minimal main.rs that compiles

2. **Phase 2: Data Models**
   - Implement all structs and enums in models.rs
   - Add Serde derives and attributes
   - Add unit tests for serialization/deserialization
   - Run `cargo test` to verify tests pass
   - Fix all `cargo clippy` warnings
   - Format with `cargo fmt`

3. **Phase 3: GET Endpoint**
   - Implement get_thread handler with hardcoded data
   - Set up route in routes.rs
   - Run `cargo check` and `cargo clippy`, fix all issues
   - Run `cargo build` to compile
   - Test with `cargo run` and curl

4. **Phase 4: SSE Utilities**
   - Implement event creation helpers in sse.rs
   - Add unit tests for event generation
   - Run `cargo test` to verify
   - Fix all `cargo clippy` warnings
   - Format with `cargo fmt`

5. **Phase 5: POST Endpoint**
   - Implement send_message handler with SSE stream
   - Add route configuration
   - Run `cargo check` and `cargo clippy`, fix all issues
   - Run `cargo build` to compile
   - Test streaming with `cargo run` and curl

6. **Phase 6: Server Integration**
   - Complete main.rs
   - Add error handling and recovery
   - Final `cargo clippy` pass - fix ALL warnings
   - Final `cargo fmt` to format all code
   - Run `cargo test` - all tests must pass
   - Run `cargo build --release` for production build
   - Final end-to-end testing with `cargo run`

**Completion Criteria for Each Phase**:
- ✓ `cargo check` passes with no errors
- ✓ `cargo clippy` passes with no warnings
- ✓ `cargo fmt` applied to all files
- ✓ `cargo test` passes (when tests exist)
- ✓ `cargo build` succeeds
- ✓ `cargo run` starts server successfully (phases 3+)

## Future Considerations

This initial implementation is designed to be extended with:
- **Database layer**: Add repository pattern for message persistence
- **LLM integration**: Replace hardcoded responses with actual LLM calls
- **Thread management**: Create/delete threads, conversation persistence
- **Authentication**: JWT or API key authentication
- **Rate limiting**: Prevent abuse of streaming endpoint
- **Metrics**: Prometheus metrics for monitoring
- **Configuration**: Environment-based config (port, host, etc.)

## Notes

- All UUIDs should be validated as v4 format
- Timestamps use ISO 8601 format (handled by chrono with serde)
- SSE keep-alive can be added with `warp::sse::keep_alive()`
- Consider connection timeout handling for SSE streams
- Hardcoded thread ID for testing: `550e8400-e29b-41d4-a716-446655440000`
