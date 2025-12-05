# Event-Sourced Agent Plan

## Overview

This plan describes the implementation of an event-sourced agent that uses MessageDB to persist conversation history and agent execution state, replacing the in-memory storage in the current `Agent` implementation.

## Goals

1. **Persistent State**: Store all conversation history and agent execution in MessageDB streams
2. **Event-Driven**: Model agent execution as a series of domain events
3. **Projectable**: Reconstruct agent state by projecting events into LLM message format
4. **Streamable**: Emit events to callers for real-time streaming (SSE use case)
5. **Concurrent-Safe**: Handle multiple requests to the same thread with optimistic concurrency control
6. **Resumable**: Support resuming interrupted agent loops

## Architecture

### Stream Design

**Stream Naming Convention**:
- Stream name: `thread:v0-{threadId}` (e.g., `thread:v0-550e8400-e29b-41d4-a716-446655440000`)
- Category: `thread:v0`
- ThreadId: UUID or any unique identifier
- Version: `v0` (allows for future schema evolution)

**Why This Design**:
- Each thread is an independent stream of events
- Easy to query all threads via category queries (`thread:v0`)
- Version prefix (`v0`) enables schema evolution without breaking existing streams
- Natural partition boundary (one stream per conversation)
- Supports concurrent conversations without interference
- Future versions (`v1`, `v2`) can have different event schemas while v0 streams remain readable

### Event Types

All events will be stored in the thread stream with the following schema:

#### 1. **UserMessageReceived**
```json
{
  "type": "UserMessageReceived",
  "data": {
    "message": "What is the weather in Tokyo?",
    "timestamp": "2025-12-04T10:30:00Z"
  },
  "metadata": {
    "userId": "user-123",
    "clientIp": "192.168.1.1"
  }
}
```

#### 2. **AgentIterationStarted**
```json
{
  "type": "AgentIterationStarted",
  "data": {
    "iteration": 1,
    "timestamp": "2025-12-04T10:30:00Z"
  }
}
```

#### 3. **LlmCallStarted**
```json
{
  "type": "LlmCallStarted",
  "data": {
    "provider": "claude",
    "model": "claude-sonnet-4-5@20250929",
    "messageCount": 5,
    "timestamp": "2025-12-04T10:30:00Z"
  }
}
```

#### 4. **LlmContentDelta**
```json
{
  "type": "LlmContentDelta",
  "data": {
    "contentBlockIndex": 0,
    "deltaType": "text",
    "text": "The current weather in Tokyo is",
    "timestamp": "2025-12-04T10:30:01Z"
  }
}
```

#### 5. **LlmToolUseStarted**
```json
{
  "type": "LlmToolUseStarted",
  "data": {
    "toolUseId": "toolu_01ABC123",
    "contentBlockIndex": 1,
    "name": "get_weather",
    "timestamp": "2025-12-04T10:30:02Z"
  }
}
```

#### 6. **LlmToolUseDelta**
```json
{
  "type": "LlmToolUseDelta",
  "data": {
    "toolUseId": "toolu_01ABC123",
    "partialJson": "{\"location\": \"Tokyo\", \"unit",
    "timestamp": "2025-12-04T10:30:02Z"
  }
}
```

#### 7. **LlmToolUseCompleted**
```json
{
  "type": "LlmToolUseCompleted",
  "data": {
    "toolUseId": "toolu_01ABC123",
    "name": "get_weather",
    "input": {
      "location": "Tokyo",
      "unit": "celsius"
    },
    "timestamp": "2025-12-04T10:30:02Z"
  }
}
```

#### 8. **LlmResponseCompleted**
```json
{
  "type": "LlmResponseCompleted",
  "data": {
    "stopReason": "tool_use",
    "contentBlocks": [
      {
        "type": "text",
        "text": "Let me check the weather for you."
      },
      {
        "type": "tool_use",
        "id": "toolu_01ABC123",
        "name": "get_weather",
        "input": {"location": "Tokyo", "unit": "celsius"}
      }
    ],
    "timestamp": "2025-12-04T10:30:02Z"
  }
}
```

#### 9. **ToolExecutionStarted**
```json
{
  "type": "ToolExecutionStarted",
  "data": {
    "toolUseId": "toolu_01ABC123",
    "name": "get_weather",
    "input": {
      "location": "Tokyo",
      "unit": "celsius"
    },
    "timestamp": "2025-12-04T10:30:03Z"
  }
}
```

#### 10. **ToolExecutionCompleted**
```json
{
  "type": "ToolExecutionCompleted",
  "data": {
    "toolUseId": "toolu_01ABC123",
    "name": "get_weather",
    "result": "{\"temperature\": 18, \"conditions\": \"partly cloudy\"}",
    "timestamp": "2025-12-04T10:30:04Z"
  }
}
```

#### 11. **ToolExecutionFailed**
```json
{
  "type": "ToolExecutionFailed",
  "data": {
    "toolUseId": "toolu_01ABC123",
    "name": "get_weather",
    "error": "API rate limit exceeded",
    "timestamp": "2025-12-04T10:30:04Z"
  }
}
```

#### 12. **AgentIterationCompleted**
```json
{
  "type": "AgentIterationCompleted",
  "data": {
    "iteration": 1,
    "hasToolUses": true,
    "timestamp": "2025-12-04T10:30:04Z"
  }
}
```

#### 13. **AgentCompleted**
```json
{
  "type": "AgentCompleted",
  "data": {
    "totalIterations": 2,
    "finalResponse": "The current weather in Tokyo is 18°C with partly cloudy skies.",
    "timestamp": "2025-12-04T10:30:10Z"
  }
}
```

#### 14. **AgentFailed**
```json
{
  "type": "AgentFailed",
  "data": {
    "error": "MaxIterationsReached",
    "details": "Exceeded maximum of 10 iterations",
    "iteration": 10,
    "timestamp": "2025-12-04T10:30:15Z"
  }
}
```

### Event Projection

**Projection Logic**: Convert stream events → LLM `Message` types

```rust
// Pseudo-code for projection
fn project_events_to_messages(events: Vec<RecordedEvent>) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut current_assistant_content = Vec::new();
    let mut current_tool_uses = HashMap::new();

    for event in events {
        match event.type {
            "UserMessageReceived" => {
                // Flush any pending assistant message
                if !current_assistant_content.is_empty() {
                    messages.push(Message::assistant(current_assistant_content));
                    current_assistant_content = Vec::new();
                }

                messages.push(Message::user(event.data.message));
            }

            "LlmResponseCompleted" => {
                // Build assistant message from contentBlocks
                current_assistant_content = event.data.contentBlocks;
            }

            "ToolExecutionCompleted" => {
                messages.push(Message::assistant(current_assistant_content.clone()));
                current_assistant_content = Vec::new();

                messages.push(Message::tool_result(
                    event.data.toolUseId,
                    event.data.result
                ));
            }

            "ToolExecutionFailed" => {
                messages.push(Message::assistant(current_assistant_content.clone()));
                current_assistant_content = Vec::new();

                messages.push(Message::tool_error(
                    event.data.toolUseId,
                    event.data.error
                ));
            }

            _ => {} // Ignore streaming deltas in projection
        }
    }

    messages
}
```

**Key Principles**:
- Only "completed" events affect projection (UserMessageReceived, LlmResponseCompleted, ToolExecution*)
- Streaming events (deltas) are for real-time display only, not projection
- Projection is idempotent and deterministic
- Can rebuild entire conversation history from events

## Implementation Plan

### Phase 1: Event Types and Schemas

**Files to Create**:
- `src/llm/agent/events.rs` - Event type definitions for both ThreadEvent and AgentEvent

**Tasks**:
1. Define all ThreadEvent data structs with serde serialization (for MessageDB storage)
2. Create enum `ThreadEvent` with all event variants
3. Define `AgentEvent` enum (for streaming to callers)
4. Implement `From<ThreadEvent>` for `WriteMessage` (MessageDB)
5. Implement `From<ThreadEvent>` for `AgentEvent` (mapping storage → stream)
6. Add event metadata helpers (timestamp, correlation IDs)

**Testing**:
- Unit tests for ThreadEvent serialization/deserialization (all 14 event types)
- Verify JSON schema matches documented format
- Test `From<ThreadEvent>` conversion to `WriteMessage`
- Test `From<ThreadEvent>` conversion to `AgentEvent` for all streamable variants
- Test metadata helpers (timestamp generation, etc.)

**Example**:
```rust
// ThreadEvent: Stored in MessageDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageReceivedData {
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ThreadEvent {
    UserMessageReceived(UserMessageReceivedData),
    AgentIterationStarted(AgentIterationStartedData),
    LlmContentDelta(LlmContentDeltaData),
    ToolExecutionStarted(ToolExecutionStartedData),
    // ... all other events
}

// AgentEvent: Emitted from agent.run() stream
#[derive(Debug, Clone)]
pub enum AgentEvent {
    UserMessage(String),
    TextDelta(String),
    ToolExecutionStarted { tool_use_id: String, name: String, input: serde_json::Value },
    ToolExecutionCompleted { tool_use_id: String, name: String, result: String },
    ToolExecutionFailed { tool_use_id: String, name: String, error: String },
    IterationStarted { iteration: usize },
    Completed,
}
```

### Phase 2: Event Projection

**Files to Create**:
- `src/llm/agent/projection.rs` - Event → Message projection logic

**Tasks**:
1. Implement `project_events_to_messages(events: Vec<Message>) -> Vec<Message>`
2. Handle all event types correctly
3. Handle edge cases (failed iterations, partial responses)

**Testing**:
- Unit tests with sample event sequences:
  - Simple Q&A (no tools): UserMessageReceived → LlmResponseCompleted → AgentCompleted
  - Single tool use: Full cycle with ToolExecutionCompleted
  - Multi-turn tool use: Multiple iterations with different tools
  - Failed tool execution: ToolExecutionFailed handling
  - Mixed content: Text + tool use in same response
  - Edge cases: Empty messages, streaming deltas (should be ignored), partial responses
- Verify idempotency: Same events → same messages every time
- Test message ordering is preserved

**Key Challenges**:
- Correctly grouping assistant content blocks
- Handling interleaved tool uses
- Maintaining correct message ordering

### Phase 3: Event Store Integration

**Files to Create**:
- `src/llm/agent/store.rs` - MessageDB read/write operations for threads

**Tasks**:
1. Create `ThreadStore` struct wrapping `MessageDbClient`
2. Implement `read_thread_events(thread_id: &str) -> Result<Vec<Message>, Error>`
3. Implement `append_event(thread_id: &str, event: ThreadEvent, expected_version: Option<i64>) -> Result<i64, Error>`
4. Implement `append_events(thread_id: &str, events: Vec<ThreadEvent>, expected_version: Option<i64>) -> Result<i64, Error>` (batch)
5. Add optimistic concurrency support via expected version

**Testing**:
- Integration tests with testcontainers (real MessageDB):
  - Read empty thread (should return empty vec)
  - Append single event, verify version returned
  - Append multiple events in batch
  - Read events back, verify order and content
  - Test optimistic concurrency: concurrent writes with same expected version (should conflict)
  - Test version tracking: write with correct expected version succeeds
  - Test stream name formatting: `thread:v0-{id}` pattern
  - Test category extraction: verify category is `thread:v0`

**Example**:
```rust
pub struct ThreadStore {
    client: MessageDbClient,
}

impl ThreadStore {
    pub async fn read_thread_events(&self, thread_id: &str) -> Result<Vec<Message>, Error> {
        let stream_name = format!("thread:v0-{}", thread_id);
        self.client.read_stream(&stream_name, None, None).await
    }

    pub async fn append_event(
        &self,
        thread_id: &str,
        event: ThreadEvent,
        expected_version: Option<i64>
    ) -> Result<i64, Error> {
        let stream_name = format!("thread:v0-{}", thread_id);
        let write_msg = WriteMessage::from(event);
        self.client.write_message(&stream_name, write_msg, expected_version).await
    }
}
```

### Phase 4: Event-Sourced Agent

**Files to Create**:
- `src/llm/agent/event_sourced.rs` - Main event-sourced agent implementation

**Tasks**:
1. Create `EventSourcedAgent` struct with:
   - `provider: Box<dyn LlmProvider>`
   - `tool_executor: Box<dyn ToolExecutor>`
   - `store: ThreadStore`
   - `config: GenerationConfig`
   - `system: Option<String>`
   - `max_iterations: usize`

2. Implement core `run()` method:
```rust
pub async fn run(
    &self,
    thread_id: String,
    user_message: String,
) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send>>, AgentError>
```

3. Flow:
   - Append `UserMessageReceived` event
   - Read all thread events
   - Project events → messages
   - Enter agent loop (similar to current Agent but with events)
   - For each LLM event, append to MessageDB AND emit to stream
   - For each tool execution, append events to MessageDB AND emit
   - Return stream of `AgentEvent` (which wraps ThreadEvents)

4. Handle concurrency conflicts (retry on optimistic lock failure)

**Testing**:
- Unit tests for event mapping:
  - Test `From<ThreadEvent>` for `AgentEvent` for all variants
  - Verify data is correctly extracted and transformed
  - Test that only streamable events are mapped (internal events should panic or be filtered)
- Unit tests with mock LLM provider and mock ThreadStore:
  - Simple conversation (no tools)
  - Single tool use flow
  - Multi-iteration loop
  - Max iterations reached
  - LLM error handling
  - Verify ThreadEvents written to store are correctly mapped to AgentEvents in stream
- Integration tests with real MessageDB and mock LLM:
  - Verify all ThreadEvents written to MessageDB
  - Verify AgentEvents emitted to stream correspond to ThreadEvents in DB
  - Test concurrent requests to same thread (optimistic lock conflicts)
  - Test resuming: run agent, verify events in DB, call again with new message
  - Verify version tracking through iterations

**Key Design Decisions**:
- **Dual output**: Events go to both MessageDB (persistence) AND returned stream (real-time)
- **Event granularity**: Store both fine-grained deltas (for streaming UX) and coarse-grained completed events (for projection)
- **Version tracking**: Track expected version through loop to detect conflicts
- **Error recovery**: On write failure, should we rollback? Or accept partial writes?
- **Separate event types**: Use `ThreadEvent` (MessageDB storage) and `AgentEvent` (returned stream) as separate types
  - **Benefits**:
    - Decouples persistence schema from API contract
    - ThreadEvent can evolve for storage needs without breaking stream consumers
    - AgentEvent can be simplified/optimized for streaming UX
    - Clear separation of concerns

**Event Type Mapping**:
```rust
// AgentEvent is what gets emitted from the stream
#[derive(Debug, Clone)]
pub enum AgentEvent {
    UserMessage(String),
    TextDelta(String),
    ToolExecutionStarted { tool_use_id: String, name: String, input: serde_json::Value },
    ToolExecutionCompleted { tool_use_id: String, name: String, result: String },
    ToolExecutionFailed { tool_use_id: String, name: String, error: String },
    IterationStarted { iteration: usize },
    Completed,
}

// Mapping from ThreadEvent (stored) to AgentEvent (streamed)
impl From<ThreadEvent> for AgentEvent {
    fn from(event: ThreadEvent) -> Self {
        match event {
            ThreadEvent::UserMessageReceived(data) => AgentEvent::UserMessage(data.message),
            ThreadEvent::LlmContentDelta(data) => AgentEvent::TextDelta(data.text),
            ThreadEvent::ToolExecutionStarted(data) => AgentEvent::ToolExecutionStarted {
                tool_use_id: data.tool_use_id,
                name: data.name,
                input: data.input,
            },
            ThreadEvent::ToolExecutionCompleted(data) => AgentEvent::ToolExecutionCompleted {
                tool_use_id: data.tool_use_id,
                name: data.name,
                result: data.result,
            },
            ThreadEvent::ToolExecutionFailed(data) => AgentEvent::ToolExecutionFailed {
                tool_use_id: data.tool_use_id,
                name: data.name,
                error: data.error,
            },
            ThreadEvent::AgentIterationStarted(data) => AgentEvent::IterationStarted {
                iteration: data.iteration,
            },
            ThreadEvent::AgentCompleted(_) => AgentEvent::Completed,
            // Other ThreadEvent variants may not map (e.g., internal events)
            _ => panic!("Unexpected event type for streaming"),
        }
    }
}
```

### Phase 5: HTTP Handler Integration

**Files to Modify**:
- `src/handlers/send_message.rs` - Use EventSourcedAgent instead of in-memory Agent
- `src/handlers/get_thread.rs` - Use ThreadStore and projection to retrieve thread history

**Tasks**:

#### POST /api/v1/threads/{threadId} (send_message.rs)
1. Inject `ThreadStore` and `EventSourcedAgent` dependencies
2. Extract threadId from request path
3. Extract user message from request body
4. Call `agent.run(thread_id, user_message)`
5. Map `AgentEvent` stream → SSE events
6. Handle errors gracefully (stream error events)

#### GET /api/v1/threads/{threadId} (get_thread.rs)
1. Inject `ThreadStore` dependency
2. Extract threadId from request path
3. Read thread events from MessageDB via `store.read_thread_events(thread_id)`
4. Project events to messages using `project_events_to_messages()`
5. Build `Thread` response with messages
6. Return JSON response with thread metadata and message history
7. Handle errors gracefully (thread not found, projection errors)

**Testing**:

#### POST Endpoint Integration Tests:
- Test POST to `/api/v1/threads/{threadId}` with message
- Verify SSE response stream format (event types: agent_text, tool_call, tool_response, done)
- Test error handling (invalid thread ID, agent errors)
- Test multiple messages to same thread (conversation continuity)
- Verify events are persisted in MessageDB after request completes
- Test SSE reconnection (client drops, events still in DB)

#### GET Endpoint Integration Tests:
- Test GET on empty thread (should return thread with empty messages array)
- Test GET after single message exchange (verify message reconstruction)
- Test GET after multi-turn conversation with tool uses
- Test GET with thread that has failed iterations (verify error messages included)
- Test GET with invalid/non-existent thread ID (should return 404 or empty thread)
- Verify message ordering is correct (chronological)
- Verify projected messages match original conversation structure
- Test performance with long threads (many events → projection time)

**Example - POST Handler**:
```rust
pub async fn send_message(
    thread_id: String,
    message: SendMessageRequest,
    agent: Arc<EventSourcedAgent>,
) -> Result<impl Reply, Rejection> {
    let event_stream = agent.run(thread_id, message.content).await
        .map_err(|e| reject::custom(AgentError(e)))?;

    let sse_stream = event_stream.map(|event| {
        match event {
            Ok(AgentEvent::TextDelta(text)) => {
                Ok(Event::default()
                    .event("agent_text")
                    .data(json!({"text": text}).to_string()))
            }
            Ok(AgentEvent::ToolExecutionStarted { name, input, .. }) => {
                Ok(Event::default()
                    .event("tool_call")
                    .data(json!({"name": name, "input": input}).to_string()))
            }
            // ... other mappings
        }
    });

    Ok(sse::reply(sse_stream))
}
```

**Example - GET Handler**:
```rust
pub async fn get_thread(
    thread_id: String,
    store: Arc<ThreadStore>,
) -> Result<impl Reply, Rejection> {
    // Read all events for this thread
    let events = store.read_thread_events(&thread_id).await
        .map_err(|e| reject::custom(StoreError(e)))?;

    // Project events to messages
    let messages = project_events_to_messages(events);

    // Build thread response
    let thread = Thread {
        id: thread_id,
        messages,
        created_at: messages.first()
            .and_then(|m| m.timestamp)
            .unwrap_or_else(|| Utc::now()),
        updated_at: messages.last()
            .and_then(|m| m.timestamp)
            .unwrap_or_else(|| Utc::now()),
    };

    Ok(warp::reply::json(&thread))
}
```

#### Send + Sync Stream Issue

Claude's implementation notes from implementing Phase 5:
⚠️ Remaining Issue

There's a Sync trait issue with the event streams. The EventSourcedAgent.run() method returns a stream that
is Send but not Sync, while warp's SSE requires streams to be both Send + Sync.

Solutions to consider:
1. Refactor EventSourcedAgent to use tokio channels (started in event_sourced.rs but incomplete due to syntax
  errors)
2. Use a different HTTP framework that doesn't require Sync streams
3. Create a Sync-compatible stream wrapper

Solution 1: Fix Tokio Channels (Minimal Change)

The current code almost works but has syntax errors. You're using yield inside an async block, which isn't
valid Rust. Instead, you need to use tx.send().

Assessment: ✅ Straightforward fix, keeps Warp

Required changes:
- Replace all yield Ok(...) with tx.send(Ok(...)).await
- Replace all yield Err(...) with tx.send(Err(...)).await
- Actually return the ReceiverStream (line 554 references undefined stream variable)

Pros:
- Minimal disruption - only fixes event_sourced.rs
- ReceiverStream is Send + Sync, satisfies Warp's requirements
- Standard Rust async pattern

Cons:
- Adds buffering layer (100-item channel capacity)
- Slightly more complex than direct streaming

### Phase 6: Optimizations

**Performance Considerations**:

1. **Snapshot Strategy** (for long threads):
   - After N events, write a `ThreadSnapshot` event with full projected state
   - On read, seek to latest snapshot then apply subsequent events
   - Reduces projection overhead for long conversations

2. **Caching**:
   - Cache projected messages per thread with version tracking
   - Invalidate on new events
   - Reduces repeated projection costs

3. **Batch Writes**:
   - Buffer multiple events during an iteration
   - Write as transaction at iteration boundaries
   - Reduces database round-trips

4. **Indexing**:
   - Consider MessageDB category projections for cross-thread queries
   - "Active threads", "threads by user", etc.

## Migration Path

### Option 1: Parallel Implementation
- Keep existing `Agent` for now
- Build `EventSourcedAgent` alongside
- Switch handlers gradually
- Deprecate old Agent when stable

### Option 2: Direct Replacement
- Replace Agent internals with event sourcing
- Keep same public API
- More disruptive but cleaner

**Recommendation**: Option 1 (parallel) for safer rollout

## Open Questions

1. **Concurrency Model**: How to handle multiple simultaneous requests to same thread?
   - Option A: Reject concurrent requests (simple, safe)
   - Option B: Queue requests per thread (complex, better UX)
   - Option C: Allow conflicts, rely on optimistic locking (simple, potential retry storms)

2. **Event Granularity**: Store every streaming delta or only completed events?
   - Current plan: Store both (deltas for UX, completed for projection)
   - Alternative: Store only completed, generate deltas on read (simpler storage, worse UX)

3. **Error Handling**: What if event write fails mid-iteration?
   - Option A: Rollback entire iteration (complex, requires transaction support)
   - Option B: Accept partial writes, mark as failed, allow retry (simpler, eventual consistency)

4. **Snapshot Strategy**: When/how to implement snapshots?
   - Wait until performance issues appear?
   - Build in from start?

5. **Event Schema Evolution**: How to handle event schema changes over time?
   - Stream naming includes `v0` prefix to support multiple schema versions
   - Future approach: Create new `thread:v1` category with updated schemas
   - Migration strategy: Keep v0 streams readable, write new threads to v1
   - Upcasting: Could build adapters to read v0 events and convert to v1 format if needed
   - For early stage: Accept breaking changes within v0, increment version for major changes

## Success Criteria

- [ ] All conversation history persisted in MessageDB
- [ ] Can reconstruct conversation from events
- [ ] HTTP handler streams events via SSE
- [ ] Supports concurrent threads (different thread IDs)
- [ ] Handles optimistic concurrency conflicts
- [ ] Unit tests for projection logic
- [ ] Integration tests with real MessageDB
- [ ] Documentation for event schemas

## Future Enhancements

1. **Replay/Debugging**: Tool to replay thread execution from events
2. **Analytics**: Query patterns across all threads (popular tools, avg iterations, etc.)
3. **Audit Trail**: Full audit log of all agent actions
4. **Multi-Agent**: Different agents working on same thread
5. **Human-in-Loop**: Pause agent, wait for human approval, resume
6. **Thread Forking**: Branch conversations at any point
7. **Event Subscriptions**: External systems react to thread events (notifications, logging, etc.)

## References

- Current Agent: `src/llm/agent/mod.rs`
- MessageDB Client: `src/message_db/`
- HTTP Handlers: `src/handlers/`
- SSE Implementation: `src/sse.rs`
- Event Sourcing Patterns: [Message DB Documentation](https://github.com/message-db/message-db)
