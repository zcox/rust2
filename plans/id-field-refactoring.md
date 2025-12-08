# ID Field Naming Refactoring Plan

## Problem Statement

The codebase currently uses generic `id` field names throughout, which creates ambiguity about what type of ID is being referenced. This violates the API design best practice of using descriptive ID field names (`message_id`, `thread_id`, etc.).

Additionally, SSE streaming events are missing critical `message_id` fields that would allow clients to correlate streamed chunks with the persisted message records.

## Current Violations

### 1. src/models.rs

#### Message struct (line 40)
```rust
pub struct Message {
    pub id: String,  // ← Should be message_id
    pub message_type: MessageType,
    pub timestamp: DateTime<Utc>,
    pub content: MessageContent,
}
```
**Change:** `id` → `message_id`
**Rationale:** This is a message identifier

#### AgentTextChunk struct (line 63)
```rust
pub struct AgentTextChunk {
    pub id: String,  // ← Currently holds thread_id, missing message_id
    pub chunk: String,
}
```
**Change:**
- `id` → `thread_id`
- Add new field: `message_id: String`

**Rationale:** Currently stores the thread ID. Needs both thread_id for context AND message_id to correlate with the persisted Message record.

#### ToolCallEvent struct (line 70)
```rust
pub struct ToolCallEvent {
    pub id: String,  // ← Should be tool_call_id
    pub tool_name: String,
    pub arguments: serde_json::Value,
}
```
**Change:** `id` → `tool_call_id`
**Rationale:** This is the unique identifier for a tool invocation from the LLM

#### ToolResultEvent struct (line 78)
```rust
pub struct ToolResultEvent {
    pub id: String,  // ← Should be tool_result_id
    pub tool_call_id: String,
    pub result: serde_json::Value,
}
```
**Change:** `id` → `tool_result_id`
**Rationale:** This is a synthetic ID for the tool result event (e.g., "result-{tool_call_id}")

---

### 2. src/llm/core/types.rs

#### ContentBlock::ToolCall variant (line 96)
```rust
ToolUse {  // ← Should be ToolCall
    id: String,  // ← Should be tool_call_id
    name: String,
    input: Value,
}
```
**Change:**
- Rename variant `ToolUse` → `ToolCall`
- Rename field `id` → `tool_call_id`

**Rationale:** Aligns with ubiquitous language of "tool call" not "tool use"

#### ContentBlock::ToolResult variant (line 102)
```rust
ToolResult {
    tool_use_id: String,  // ← Should be tool_call_id
    content: String,
    is_error: bool,
}
```
**Change:** `tool_use_id` → `tool_call_id`
**Rationale:** Consistent naming with tool call terminology

#### MessageMetadata struct (line 163)
```rust
pub struct MessageMetadata {
    pub id: String,  // ← Should be message_id
    pub model: String,
    pub usage: Usage,
    pub stop_reason: Option<StopReason>,
}
```
**Change:** `id` → `message_id`
**Rationale:** This is the LLM provider's message ID (e.g., Claude's `msg_01ABC...`). Context makes it clear this is different from application-level message IDs

#### ContentBlockStart::ToolCall variant (line 177)
```rust
ToolUse {  // ← Should be ToolCall
    id: String,  // ← Should be tool_call_id
    name: String,
}
```
**Change:**
- Rename variant `ToolUse` → `ToolCall`
- Rename field `id` → `tool_call_id`

**Rationale:** Aligns with ubiquitous language and identifies tool call in streaming context

#### PartialToolCall struct (line 192)
```rust
pub struct PartialToolUse {  // ← Should be PartialToolCall
    pub id: Option<String>,  // ← Should be tool_call_id
    pub name: Option<String>,
    pub partial_json: String,
}
```
**Change:**
- Rename struct `PartialToolUse` → `PartialToolCall`
- Rename field `id` → `tool_call_id`

**Rationale:** Aligns with ubiquitous language; identifier for tool call being accumulated during streaming

---

## Migration Plan

### Phase 1: Update Type Definitions

1. **src/models.rs**
   - Rename `Message.id` → `Message.message_id`
   - Rename `AgentTextChunk.id` → `AgentTextChunk.thread_id`
   - Add `AgentTextChunk.message_id: String`
   - Rename `ToolCallEvent.id` → `ToolCallEvent.tool_call_id`
   - Rename `ToolResultEvent.id` → `ToolResultEvent.tool_result_id`
   - Rename `MessageType::ToolResponse` → `MessageType::ToolResult`
   - Rename `MessageContent::ToolResponse` → `MessageContent::ToolResult`

2. **src/llm/core/types.rs**
   - Rename `ContentBlock::ToolUse` → `ContentBlock::ToolCall`
   - Rename `ContentBlock::ToolCall.id` → `tool_call_id`
   - Rename `ContentBlock::ToolResult.tool_use_id` → `tool_call_id`
   - Rename `MessageMetadata.id` → `message_id`
   - Rename `ContentBlockStart::ToolUse` → `ContentBlockStart::ToolCall`
   - Rename `ContentBlockStart::ToolCall.id` → `tool_call_id`
   - Rename `PartialToolUse` → `PartialToolCall`
   - Rename `PartialToolCall.id` → `tool_call_id`

### Phase 2: Update SSE Event Creation

**File:** `src/sse.rs`

Rename `create_tool_response_event()` → `create_tool_result_event()`

Update SSE event type: `"tool_response"` → `"tool_result"`

Update `create_agent_text_event()` signature:
```rust
pub fn create_agent_text_event(
    thread_id: String,
    message_id: String,  // ← Add this parameter
    chunk: String,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "thread_id": thread_id,
        "message_id": message_id,  // ← Include in payload
        "chunk": chunk
    });
    // ...
}
```

Update `create_tool_call_event()`:
```rust
pub fn create_tool_call_event(
    tool_call_id: String,  // ← Rename parameter
    tool_name: String,
    arguments: Value,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "tool_call_id": tool_call_id,  // ← Rename field
        "tool_name": tool_name,
        "arguments": arguments
    });
    // ...
}
```

Update `create_tool_result_event()`:
```rust
pub fn create_tool_result_event(
    tool_result_id: String,  // ← Rename parameter
    tool_call_id: String,  // ← Rename parameter
    result: Value,
) -> Result<Event, std::convert::Infallible> {
    let payload = serde_json::json!({
        "tool_result_id": tool_result_id,  // ← Rename field
        "tool_call_id": tool_call_id,  // ← Rename field
        "result": result
    });

    Ok(Event::default()
        .event("tool_result")  // ← Changed from "tool_response"
        .data(payload.to_string()))
}
```

### Phase 3: Update Handler to Generate and Pass message_id

**File:** `src/handlers/send_message.rs`

The handler needs to:
1. Generate a message_id when starting the agent response
2. Pass it to all SSE event creation calls

```rust
pub async fn send_message_handler(
    thread_id: Uuid,
    request: SendMessageRequest,
    agent: Arc<EventSourcedAgent>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let thread_id_str = thread_id.to_string();

    // Generate message ID for this agent response
    let message_id = format!("msg_{}", Uuid::new_v4());

    let agent_stream = agent.run(thread_id_str.clone(), request.text.clone()).await?;

    let sse_stream = agent_stream.map(move |event_result| {
        let msg_id = message_id.clone();  // Clone for each event
        match event_result {
            Ok(agent_event) => match agent_event {
                AgentEvent::TextDelta(text) => {
                    create_agent_text_event(
                        thread_id.to_string(),
                        msg_id,  // ← Pass message_id
                        text
                    )
                }
                AgentEvent::ToolExecutionStarted { tool_call_id, name, input } => {
                    create_tool_call_event(tool_call_id, name, input)
                }
                // ... rest of mappings
            }
        }
    });
    // ...
}
```

### Phase 4: Update Agent to Use message_id

**File:** `src/llm/agent/mod.rs` (or wherever EventSourcedAgent is defined)

The agent needs to:
1. Generate message_id when creating agent messages
2. Store it in event-sourced events
3. Return it in the Message records

This may require updates to:
- Event schemas (MessageSent, MessageAdded, etc.)
- Message reconstruction from events
- GET handler to return messages with proper message_id field

### Phase 5: Update All Call Sites

Search for all uses of these structs and update field access:
- `.id` → `.message_id` (for Message)
- `.id` → `.thread_id` (for AgentTextChunk)
- `.id` → `.tool_call_id` (for tool-related types)
- `.id` → `.message_id` (for MessageMetadata)

### Phase 6: Update Tests

All tests in:
- `src/models.rs` (tests module)
- `src/sse.rs` (tests module)
- `src/llm/core/types.rs` (tests module)
- Integration tests

Update test assertions and JSON parsing to use new field names.

### Phase 7: Update Documentation

1. **API.md** - Update all example JSON payloads
2. **README.md** - Update SSE event format documentation
3. **CLAUDE.md** - Add examples showing the new field names

---

## Breaking Changes

This is a **breaking change** for API clients. The JSON schema changes:

### Before:
```json
{
  "event": "agent_text",
  "data": {
    "id": "thread-uuid",
    "chunk": "Hello"
  }
}
```

### After:
```json
{
  "event": "agent_text",
  "data": {
    "thread_id": "thread-uuid",
    "message_id": "msg_abc123",
    "chunk": "Hello"
  }
}
```

Since the user has stated they'll delete all MessageDB data and start over, we don't need backward compatibility.

---

## Implementation Checklist

- [ ] Phase 1: Update all struct definitions
- [ ] Phase 2: Update SSE event creation functions
- [ ] Phase 3: Update send_message handler to generate and pass message_id
- [ ] Phase 4: Update agent to generate and store message_id in events
- [ ] Phase 5: Update all call sites throughout codebase
- [ ] Phase 6: Update all tests
- [ ] Phase 7: Update documentation
- [ ] Run `cargo fmt`
- [ ] Run `cargo clippy`
- [ ] Run `cargo test`
- [ ] Manual testing with curl/client
- [ ] Delete MessageDB data: `docker-compose down -v && docker-compose up -d`

---

## Estimated Impact

**Files to modify:**
- src/models.rs
- src/sse.rs
- src/handlers/send_message.rs
- src/handlers/get_thread.rs (if it constructs Message objects)
- src/llm/core/types.rs
- src/llm/agent/ (event schemas and message reconstruction)
- All test files
- Documentation files

**Test files to update:**
- Unit tests in models.rs
- Unit tests in sse.rs
- Unit tests in llm/core/types.rs
- Integration tests
- Example assertions

---

## Open Questions

1. **Message ID Generation Strategy**
   - Use Uuid::new_v4() with "msg_" prefix?
   - Use sequential IDs?
   - Use timestamp-based IDs?

   **Recommendation:** `msg_{uuid}` for global uniqueness

2. **Event Sourcing Schema**
   - Do existing events in MessageDB store message IDs?
   - What's the current event schema for MessageSent/MessageAdded?
   - Need to review agent implementation to ensure message_id is persisted

3. **GET /threads/{threadId} Response**
   - Does it currently return messages with id field?
   - Need to verify the get_thread handler implementation

---

## Success Criteria

After this refactoring:
1. ✅ No struct fields named just `id`
2. ✅ All ID fields have descriptive names
3. ✅ SSE agent_text events include both thread_id and message_id
4. ✅ Clients can correlate streamed chunks to persisted messages
5. ✅ All tests pass
6. ✅ API is self-documenting with clear field names
