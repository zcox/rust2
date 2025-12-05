# Phase 5 Automated Test Plan

## Overview

This document details all automated tests needed for Phase 5 (HTTP Handler Integration) of the event-sourced agent implementation. These tests ensure the POST and GET endpoints work correctly with MessageDB persistence and SSE streaming.

## Current Test Coverage Status

### ✅ Completed (Phase 3 - ThreadStore)
- `tests/agent_store_test.rs` - 15 comprehensive tests for ThreadStore operations
  - Basic CRUD operations
  - Batch operations
  - Optimistic concurrency control
  - Stream naming and versioning
  - All 14 event types roundtrip
  - Category verification

### ❌ Missing (Phase 5 - HTTP Handlers)
- **0 tests** currently exist for HTTP handler layer
- **0 tests** for end-to-end flows
- **0 tests** for real LLM integration

---

## Required Test Files

### 1. Handler Integration Tests
**File:** `tests/handler_integration_test.rs`

Integration tests for POST and GET handlers using real MessageDB (testcontainers) with mock LLM provider.

#### Setup Pattern
```rust
use testcontainers::clients::Cli;
use rust2::llm::agent::{EventSourcedAgent, ThreadStore};
use rust2::message_db::{MessageDbClient, MessageDbConfig};
use warp::test::request;

// Mock LLM provider that returns predictable responses
struct TestLlmProvider {
    responses: Vec<String>,
}

// Test fixture setup
async fn setup_test_env() -> (EventSourcedAgent, ThreadStore, testcontainers::Container) {
    // Start MessageDB container
    // Create client, store, agent
    // Return ready-to-use instances
}
```

#### POST Endpoint Tests

##### Test: `test_post_message_basic`
**Purpose:** Verify basic POST message handling and SSE response

**Steps:**
1. Start MessageDB container with testcontainers
2. Create EventSourcedAgent with mock LLM (returns simple text response)
3. POST to `/api/v1/threads/{uuid}` with `{"text": "Hello"}`
4. Consume SSE stream, collect events
5. Verify response contains:
   - `agent_text` events with expected text
   - `done` event at end
6. Query MessageDB directly to verify events persisted:
   - `UserMessageReceived`
   - `AgentIterationStarted`
   - `LlmCallStarted`
   - `LlmContentDelta` events
   - `LlmResponseCompleted`
   - `AgentIterationCompleted`
   - `AgentCompleted`

**Success Criteria:**
- SSE stream completes without errors
- All expected events received
- Events match expected order
- MessageDB contains all events

##### Test: `test_post_message_sse_event_format`
**Purpose:** Verify SSE event format compliance

**Steps:**
1. Setup test environment
2. POST message to thread
3. Parse raw SSE response
4. Verify each event has correct structure:
   ```
   event: agent_text
   data: {"thread_id": "...", "text": "..."}

   event: done
   data: {}
   ```

**Success Criteria:**
- All events have `event:` and `data:` fields
- JSON in `data:` field is valid
- Event types match spec: `agent_text`, `tool_call`, `tool_response`, `done`
- Thread ID included in relevant events

##### Test: `test_post_message_with_tool_calls`
**Purpose:** Verify tool calling flow through SSE

**Steps:**
1. Create mock LLM that returns tool use:
   ```rust
   StreamEvent::ContentBlockStart {
       index: 0,
       block: ContentBlockStart::ToolUse {
           id: "toolu_123",
           name: "calculator",
       }
   }
   ```
2. Create mock tool executor that returns result
3. POST message that triggers tool use
4. Verify SSE stream contains:
   - `agent_text` (optional pre-tool text)
   - `tool_call` with tool name and input
   - `tool_response` with result
   - `agent_text` (optional post-tool text)
   - `done`
5. Verify MessageDB contains:
   - `LlmToolUseStarted`
   - `LlmToolUseCompleted`
   - `ToolExecutionStarted`
   - `ToolExecutionCompleted`

**Success Criteria:**
- Tool call event has correct structure
- Tool response event includes tool_use_id
- MessageDB has complete tool execution trail

##### Test: `test_post_message_error_handling`
**Purpose:** Verify error scenarios handled gracefully

**Scenarios to test:**
1. **Mock LLM returns error:**
   - Mock provider returns `Err(LlmError::ApiError(...))`
   - Verify SSE stream includes error event
   - Verify `AgentFailed` event in MessageDB

2. **Tool execution fails:**
   - Mock tool executor returns `Err("Tool failed")`
   - Verify `tool_response` event includes error
   - Verify `ToolExecutionFailed` event in MessageDB

3. **Max iterations reached:**
   - Mock LLM always returns tool use (infinite loop)
   - Verify agent stops at max iterations
   - Verify `AgentFailed` event with "MaxIterationsReached"

**Success Criteria:**
- No panics or crashes
- Error events properly formatted
- MessageDB contains failure events
- SSE stream closes gracefully

##### Test: `test_post_multiple_messages_same_thread`
**Purpose:** Verify conversation continuity

**Steps:**
1. Setup test environment
2. POST message 1: "What is 2+2?"
3. Wait for completion (consume SSE stream)
4. POST message 2: "What about 3+3?" (same thread ID)
5. Verify second response has context of first message
6. Query MessageDB for thread
7. Verify event sequence shows both conversations

**Success Criteria:**
- Both user messages present in MessageDB
- Events in chronological order
- Second agent response acknowledges context
- No events from other threads mixed in

##### Test: `test_post_message_persistence_verification`
**Purpose:** Verify events persist correctly

**Steps:**
1. Setup test environment
2. POST message with known content
3. Consume SSE stream to completion
4. Read events directly from MessageDB using ThreadStore
5. Verify all expected events present:
   - Correct event types
   - Correct event data
   - Correct ordering
   - Correct stream name format (`thread:v0-{id}`)

**Success Criteria:**
- Event count matches expectations
- Event data matches what was streamed
- No duplicate events
- Stream version increments correctly

##### Test: `test_post_concurrent_different_threads`
**Purpose:** Verify thread isolation

**Steps:**
1. Setup test environment
2. Spawn 3 concurrent POST requests to different thread IDs
3. Let all complete
4. Verify each thread has only its own events
5. Verify no cross-contamination

**Success Criteria:**
- All 3 requests complete successfully
- Each thread has exactly its own events
- No events from thread A in thread B's stream

#### GET Endpoint Tests

##### Test: `test_get_empty_thread`
**Purpose:** Verify GET on non-existent thread

**Steps:**
1. Setup test environment
2. Generate random UUID (don't write any events)
3. GET `/api/v1/threads/{uuid}`
4. Verify response structure

**Success Criteria:**
- Status code: 200 (or 404, depending on design choice)
- Response has `thread_id` and `messages` fields
- `messages` array is empty
- No errors or panics

##### Test: `test_get_thread_after_single_message`
**Purpose:** Verify message reconstruction from events

**Steps:**
1. Setup test environment
2. Write events to MessageDB for simple Q&A:
   - `UserMessageReceived`: "Hello"
   - `LlmResponseCompleted`: "Hi there!"
   - `AgentCompleted`
3. GET `/api/v1/threads/{thread_id}`
4. Verify response contains 2 messages:
   - User message: "Hello"
   - Assistant message: "Hi there!"

**Success Criteria:**
- Exactly 2 messages in response
- Message types correct (User, Agent)
- Message content matches events
- Message order correct (user first, assistant second)

##### Test: `test_get_thread_with_tool_use`
**Purpose:** Verify tool use reconstruction

**Steps:**
1. Write events to MessageDB for conversation with tool:
   - `UserMessageReceived`: "What's the weather?"
   - `LlmResponseCompleted`: [text: "Let me check", tool_use: get_weather]
   - `ToolExecutionCompleted`: {"temp": 72}
   - `LlmResponseCompleted`: "It's 72°F"
   - `AgentCompleted`
2. GET thread
3. Verify projected messages:
   - Message 1: User - "What's the weather?"
   - Message 2: Assistant - "Let me check"
   - Message 3: Tool result - {"temp": 72}
   - Message 4: Assistant - "It's 72°F"

**Success Criteria:**
- All messages present
- Tool use properly represented
- Message types correct
- Order preserved

##### Test: `test_get_thread_multi_turn_conversation`
**Purpose:** Verify complex conversation reconstruction

**Steps:**
1. Write events for 3-turn conversation:
   - Turn 1: User asks, agent responds
   - Turn 2: User follows up, agent responds
   - Turn 3: User asks new question, agent responds
2. GET thread
3. Verify 6 messages total (3 user + 3 assistant)
4. Verify chronological order
5. Verify each message has correct content

**Success Criteria:**
- All 6 messages present
- Order is chronological
- No missing or duplicate messages
- Content matches original events

##### Test: `test_get_thread_with_failed_iteration`
**Purpose:** Verify error handling in projection

**Steps:**
1. Write events including failure:
   - `UserMessageReceived`
   - `AgentIterationStarted`
   - `LlmCallStarted`
   - `AgentFailed`: "MaxIterationsReached"
2. GET thread
3. Verify response doesn't crash
4. Verify error is represented somehow (design choice)

**Success Criteria:**
- No 500 error
- Response is valid JSON
- User message present
- Failure indicated in some way

##### Test: `test_get_thread_message_ordering`
**Purpose:** Verify projection maintains order

**Steps:**
1. Write many events (20+) representing complex conversation
2. GET thread
3. Verify messages are in strict chronological order
4. Verify no messages are out of sequence

**Success Criteria:**
- Message timestamps increase monotonically
- User/assistant alternation preserved (where applicable)
- No ordering bugs

##### Test: `test_get_thread_performance_long_thread`
**Purpose:** Verify projection performance doesn't degrade

**Steps:**
1. Write 100 events to thread (10 turn conversation with tools)
2. Measure time to GET thread
3. Verify response time

**Success Criteria:**
- Response time < 1 second
- All messages present
- No timeouts
- Memory usage reasonable

---

### 2. End-to-End Tests
**File:** `tests/e2e_test.rs`

Full stack tests with HTTP server running, MessageDB container, and mock LLM.

#### Setup Pattern
```rust
use warp::Filter;
use reqwest::Client;

async fn start_test_server() -> (String, /* cleanup handle */) {
    // Start MessageDB
    // Create agent with mocks
    // Start warp server on random port
    // Return URL like "http://127.0.0.1:54321"
}

async fn stop_test_server(/* cleanup handle */) {
    // Shutdown server
    // Cleanup containers
}
```

#### Tests

##### Test: `test_e2e_post_then_get`
**Purpose:** Verify complete flow from POST to GET

**Steps:**
1. Start test server on random port
2. Use `reqwest` to POST message
3. Consume SSE stream via reqwest
4. Collect all events from stream
5. Use `reqwest` to GET same thread
6. Verify GET response matches SSE stream content

**Success Criteria:**
- POST returns SSE stream
- GET returns messages that match stream
- Data persisted correctly

##### Test: `test_e2e_concurrent_threads`
**Purpose:** Verify thread isolation at HTTP level

**Steps:**
1. Start test server
2. Spawn 3 concurrent reqwest POST requests to different threads
3. Consume all 3 SSE streams concurrently
4. GET all 3 threads
5. Verify no interference

**Success Criteria:**
- All 3 threads complete independently
- Each thread has only its own messages
- No cross-talk

##### Test: `test_e2e_multi_turn_conversation`
**Purpose:** Verify conversation flow

**Steps:**
1. Start test server
2. POST message 1, consume stream
3. POST message 2 to same thread, consume stream
4. POST message 3 to same thread, consume stream
5. GET thread
6. Verify all 6 messages (3 user + 3 assistant)

**Success Criteria:**
- Conversation builds correctly
- GET shows complete history
- Order preserved

##### Test: `test_e2e_sse_stream_consumption`
**Purpose:** Verify SSE stream is consumable by HTTP client

**Steps:**
1. Start test server
2. Use reqwest with SSE support to POST message
3. Read events as they arrive (don't wait for completion)
4. Verify events arrive incrementally

**Success Criteria:**
- Events arrive in chunks (streaming works)
- Not all events arrive at once
- Stream closes properly with `done` event

##### Test: `test_e2e_error_recovery`
**Purpose:** Verify error handling at HTTP level

**Steps:**
1. Start test server
2. POST with mock LLM configured to fail
3. Verify HTTP response includes error event
4. GET thread to verify state
5. POST again to same thread (recovery)
6. Verify second attempt works

**Success Criteria:**
- Error doesn't crash server
- State is consistent
- Thread can be used after error

---

### 3. Real LLM Integration Tests
**File:** `tests/e2e_real_llm_test.rs`

Tests with actual Claude/Gemini API. These are marked `#[ignore]` and only run explicitly with credentials configured.

#### Environment Setup
```rust
// Requires:
// - GCP_PROJECT_ID env var
// - GCP Application Default Credentials configured
// - Vertex AI API enabled

#[tokio::test]
#[ignore] // Run with: cargo test --test e2e_real_llm_test -- --ignored
```

#### Tests

##### Test: `test_real_llm_simple_conversation`
**Purpose:** Verify integration with real LLM API

**Steps:**
1. Check for credentials (skip if not available)
2. Start MessageDB container
3. Create EventSourcedAgent with REAL Claude client
4. Start HTTP server
5. POST: "What is 2+2? Just answer with the number."
6. Consume SSE stream
7. Verify stream contains actual LLM response
8. GET thread
9. Verify conversation stored correctly

**Success Criteria:**
- Real LLM responds (answer contains "4")
- SSE stream works with real API
- Response time reasonable (< 10s)
- Events stored in MessageDB

##### Test: `test_real_llm_with_tools`
**Purpose:** Verify tool calling with real LLM

**Steps:**
1. Configure agent with calculator tool
2. POST: "What is 15 times 23? Use the calculator."
3. Verify SSE stream shows:
   - Text about using calculator
   - tool_call event for calculator
   - tool_response with 345
   - Final answer text
4. GET thread
5. Verify complete tool cycle stored

**Success Criteria:**
- LLM decides to use tool
- Tool executes correctly
- Final answer includes result (345)
- All events in MessageDB

##### Test: `test_real_llm_streaming_quality`
**Purpose:** Verify streaming behavior with real LLM

**Steps:**
1. Setup with real Claude
2. POST: "Write a haiku about coding"
3. Measure time between first and last SSE event
4. Verify events arrive incrementally (not all at once)

**Success Criteria:**
- Events stream over time (not single chunk)
- Total time > 1 second (proves streaming)
- Response is actual haiku
- Quality acceptable

---

### 4. TODO Load and Stress Tests
**File:** `tests/load_test.rs`

Performance and concurrency tests. Marked `#[ignore]` to run separately.

#### Tests

##### Test: `test_concurrent_requests_same_thread`
**Purpose:** Verify optimistic locking under contention

**Steps:**
1. Setup test environment
2. Spawn 2 concurrent POST requests to SAME thread ID
3. Observe behavior
4. Verify optimistic locking handles conflict

**Success Criteria:**
- No data corruption
- At least one request succeeds
- Errors are graceful (retry or clear failure)
- Final thread state is consistent

##### Test: `test_many_concurrent_threads`
**Purpose:** Verify scalability

**Steps:**
1. Setup test environment
2. POST to 50 different threads concurrently
3. Wait for all to complete
4. GET all 50 threads
5. Verify all succeeded

**Success Criteria:**
- All 50 threads complete successfully
- No timeouts
- No database connection errors
- MessageDB handles load

##### Test: `test_very_long_thread`
**Purpose:** Verify performance with large thread

**Steps:**
1. Simulate 50-turn conversation (POST 50 times)
2. Measure GET request time
3. Verify projection still fast

**Success Criteria:**
- GET completes in < 2 seconds even with 100+ events
- No memory issues
- Response complete and correct

---

## Test Execution Guide

### Running Tests

```bash
# Run all unit and integration tests (except ignored)
cargo test

# Run specific test file
cargo test --test handler_integration_test

# Run specific test
cargo test --test e2e_test test_e2e_post_then_get

# Run ignored tests (real LLM)
cargo test --test e2e_real_llm_test -- --ignored

# Run load tests
cargo test --test load_test -- --ignored

# Run with output visible
cargo test -- --nocapture
```

### Prerequisites

**For integration tests:**
- Docker running (for testcontainers)

**For real LLM tests:**
- GCP project with Vertex AI enabled
- Application Default Credentials configured:
  ```bash
  gcloud auth application-default login
  export GCP_PROJECT_ID=your-project-id
  ```

**For load tests:**
- Sufficient system resources
- MessageDB container with tuned settings

---

## Test Implementation Priority

### Phase 1 (Critical - Week 1)
1. `test_post_message_basic` - Basic POST flow works
2. `test_get_empty_thread` - Basic GET works
3. `test_get_thread_after_single_message` - Projection works
4. `test_e2e_post_then_get` - Full stack works

### Phase 2 (Important - Week 2)
5. `test_post_message_with_tool_calls` - Tool use works
6. `test_post_multiple_messages_same_thread` - Conversation continuity
7. `test_get_thread_with_tool_use` - Tool projection works
8. `test_post_message_error_handling` - Error cases covered

### Phase 3 (Nice to Have - Week 3)
9. `test_get_thread_multi_turn_conversation` - Complex scenarios
10. `test_e2e_concurrent_threads` - Isolation verified
11. `test_post_message_sse_event_format` - Format compliance
12. `test_get_thread_performance_long_thread` - Performance baseline

### Phase 4 (Validation - Before Production)
13. Real LLM tests - Actual API integration verified
14. Load tests - Performance under stress known

---

## Success Metrics

- **Coverage:** All Phase 5 handler code covered by tests
- **Reliability:** Tests pass consistently (> 99% success rate)
- **Speed:** Test suite completes in < 5 minutes (excluding load tests)
- **Clarity:** Test failures clearly indicate what broke
- **Maintainability:** Tests are easy to update when code changes

---

## Related Documents

- See `docs/manual-testing-guide.md` for manual test scenarios
- See `plans/event-sourced-agent.md` for overall Phase 5 design
- See `tests/agent_store_test.rs` for Phase 3 test examples
