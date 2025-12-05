# Manual Testing Guide - Event-Sourced Agent API

## Overview

This guide covers manual testing scenarios that cannot be fully automated or require human observation/judgment. These tests complement the automated test suite and are essential for validating production readiness.

**When to use this guide:**
- Before major releases
- After significant infrastructure changes
- When debugging user-reported issues
- For quality validation of LLM responses

---

## Prerequisites

### Required Setup
- Docker and docker-compose installed
- Rust toolchain (cargo)
- MessageDB running: `docker-compose up -d`
- For real LLM tests: GCP credentials configured

### Optional Tools
- Browser DevTools (for SSE inspection)
- `curl` or `httpie` (for API testing)
- `websocat` or similar (for debugging streams)
- PostgreSQL client (for MessageDB inspection)

---

## Test 1: SSE Streaming User Experience

**Objective:** Verify that Server-Sent Events stream data incrementally to the client in real-time, providing a good user experience.

**Why manual:** Need to observe timing and perceived responsiveness, which is subjective.

### Setup

1. Start the server:
   ```bash
   cargo run
   ```

2. Open browser (Chrome, Firefox, or Safari)

3. Open DevTools → Console

### Test Procedure

#### Part A: Basic Streaming

1. Create a new thread ID:
   ```javascript
   const threadId = crypto.randomUUID();
   console.log('Thread ID:', threadId);
   ```

2. Set up EventSource listener:
   ```javascript
   const es = new EventSource(`http://127.0.0.1:3030/api/v1/threads/${threadId}`);

   let textChunks = [];
   let firstEventTime = null;
   let lastEventTime = null;

   es.addEventListener('agent_text', e => {
     const data = JSON.parse(e.data);
     const now = Date.now();

     if (!firstEventTime) firstEventTime = now;
     lastEventTime = now;

     textChunks.push(data.text);
     console.log('Text chunk:', data.text);
   });

   es.addEventListener('tool_call', e => {
     const data = JSON.parse(e.data);
     console.log('Tool call:', data.name, data.input);
   });

   es.addEventListener('tool_response', e => {
     const data = JSON.parse(e.data);
     console.log('Tool response:', data.result);
   });

   es.addEventListener('done', e => {
     const totalTime = lastEventTime - firstEventTime;
     console.log('Done! Total streaming time:', totalTime, 'ms');
     console.log('Full response:', textChunks.join(''));
     es.close();
   });

   es.addEventListener('error', e => {
     console.error('SSE error:', e);
   });
   ```

3. Send a message (in separate console or via Network tab):
   ```javascript
   fetch(`http://127.0.0.1:3030/api/v1/threads/${threadId}`, {
     method: 'POST',
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({
       text: "Tell me a creative short story about a robot learning to paint"
     })
   });
   ```

### Success Criteria

- ✅ **Incremental delivery:** Text chunks arrive over time (not all at once)
- ✅ **Timing:** Total streaming time > 0ms (proves chunks arrive separately)
- ✅ **Completeness:** All chunks combine to form complete response
- ✅ **Event order:** `agent_text` events → `done` event
- ✅ **No gaps:** No missing text between chunks
- ✅ **Connection stability:** EventSource doesn't disconnect/reconnect
- ✅ **Console output:** Each chunk logged as it arrives

### Observations to Record

| Aspect | Observation |
|--------|-------------|
| First chunk latency | Time from POST to first `agent_text` event |
| Total streaming time | Time from first to last event |
| Number of chunks | Count of `agent_text` events |
| Average chunk size | Characters per chunk |
| Perceived responsiveness | Does it feel "snappy"? |

### Troubleshooting

**Problem:** All text arrives in one chunk
- Check: Is the mock LLM emitting multiple StreamEvents?
- Check: Is the SSE serialization batching events?

**Problem:** EventSource shows "connecting" repeatedly
- Check: Is CORS configured correctly?
- Check: Are SSE headers correct? (`Content-Type: text/event-stream`)

**Problem:** No events arrive
- Check: Server logs for errors
- Check: Network tab - is request still pending?

---

## Test 2: Browser Compatibility

**Objective:** Verify SSE works across major browsers and handles browser-specific quirks.

**Why manual:** Browser behavior varies, need real testing in each environment.

### Browsers to Test

- Google Chrome (latest)
- Mozilla Firefox (latest)
- Safari (if on macOS)
- Edge (optional)

### Test Procedure (Repeat for Each Browser)

1. Start server: `cargo run`

2. Open browser DevTools → Network tab, filter by "EventSource"

3. Run the EventSource test from Test 1

4. Observe behavior in Network tab

### Success Criteria (Per Browser)

| Browser | SSE Connection | Events Arrive | Connection Closes | Notes |
|---------|----------------|---------------|-------------------|-------|
| Chrome  | ✅ | ✅ | ✅ | |
| Firefox | ✅ | ✅ | ✅ | |
| Safari  | ✅ | ✅ | ✅ | |
| Edge    | ✅ | ✅ | ✅ | |

### Browser-Specific Checks

**Chrome:**
- DevTools → Network → EventSource tab shows events
- Connection shows as "pending" while active
- Closes cleanly when `done` received

**Firefox:**
- Network tab shows event stream
- No CORS warnings in console
- Response tab shows formatted SSE

**Safari:**
- EventSource establishes (Safari can be strict on CORS)
- No unexpected disconnects
- Performance is acceptable

### Known Browser Issues

Document any browser-specific issues found:

```
Example:
- Safari 16.x: Requires explicit CORS headers even for localhost
- Firefox: Shows "NS_ERROR_NET_PARTIAL_TRANSFER" on normal SSE close
```

---

## Test 3: Network Interruption Recovery

**Objective:** Verify that data persists in MessageDB even if the SSE connection drops, and clients can recover thread state.

**Why manual:** Difficult to reliably simulate network issues in automated tests.

### Test Procedure

1. Start server and MessageDB:
   ```bash
   docker-compose up -d
   cargo run
   ```

2. Create new thread and start streaming:
   ```javascript
   const threadId = crypto.randomUUID();
   const es = new EventSource(`http://127.0.0.1:3030/api/v1/threads/${threadId}`);

   es.addEventListener('agent_text', e => console.log('Chunk:', JSON.parse(e.data).text));

   fetch(`http://127.0.0.1:3030/api/v1/threads/${threadId}`, {
     method: 'POST',
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({ text: "Write a long essay about event sourcing" })
   });
   ```

3. **While streaming is in progress**, simulate network interruption:
   - **Option A:** Disconnect WiFi/Ethernet
   - **Option B:** Kill server process (`Ctrl+C`)
   - **Option C:** Pause network in DevTools (Network tab → Throttling → Offline)

4. Wait 5-10 seconds

5. Restore connection (reconnect WiFi, restart server, or unpause network)

6. Attempt to GET the thread:
   ```javascript
   fetch(`http://127.0.0.1:3030/api/v1/threads/${threadId}`)
     .then(r => r.json())
     .then(data => console.log('Retrieved thread:', data));
   ```

### Success Criteria

- ✅ **Data persisted:** GET request returns thread with messages
- ✅ **No data loss:** All events written before disconnect are in MessageDB
- ✅ **Consistent state:** Thread state is valid (no partial/corrupt events)
- ✅ **Recoverable:** Client can continue conversation by POST-ing new message

### Recovery Scenarios

**Scenario A: Disconnect during streaming**
- Expected: Events already written persist
- Verify: GET shows partial conversation up to disconnect point

**Scenario B: Server crashes mid-write**
- Expected: MessageDB transaction ensures consistency
- Verify: No partial events in stream

**Scenario C: Client drops connection**
- Expected: Server continues writing to MessageDB
- Verify: GET shows complete response even though SSE disconnected

### Observations

| Scenario | Events Lost? | State Consistent? | Recovery Possible? | Notes |
|----------|--------------|-------------------|-------------------|-------|
| WiFi disconnect | | | | |
| Server crash | | | | |
| Client disconnect | | | | |

---

## Test 4: Long-Running Conversation

**Objective:** Verify system stability during extended multi-turn conversations.

**Why manual:** Time-intensive test requiring sustained operation.

### Test Procedure

1. Start server with monitoring:
   ```bash
   # In terminal 1: Start server
   cargo run

   # In terminal 2: Monitor resource usage
   watch -n 5 'ps aux | grep rust2'
   ```

2. Write a script to conduct 20-turn conversation:
   ```javascript
   const threadId = crypto.randomUUID();

   async function sendMessage(text) {
     const response = await fetch(`http://127.0.0.1:3030/api/v1/threads/${threadId}`, {
       method: 'POST',
       headers: { 'Content-Type': 'application/json' },
       body: JSON.stringify({ text })
     });

     // Consume SSE stream
     const reader = response.body.getReader();
     const decoder = new TextDecoder();

     while (true) {
       const { value, done } = await reader.read();
       if (done) break;

       const chunk = decoder.decode(value);
       console.log(chunk);
     }
   }

   // Conduct 20-turn conversation
   const questions = [
     "What is event sourcing?",
     "How does it differ from CRUD?",
     "What are the benefits?",
     "What are the drawbacks?",
     "When should I use it?",
     // ... add 15 more questions
   ];

   for (const q of questions) {
     console.log('User:', q);
     await sendMessage(q);
     console.log('---');
   }
   ```

3. Monitor during execution:
   - Memory usage (should not grow unbounded)
   - Response time (should not degrade significantly)
   - Server logs (check for errors)

4. After completion, GET thread:
   ```bash
   curl http://127.0.0.1:3030/api/v1/threads/{threadId}
   ```

### Success Criteria

- ✅ **Stability:** No crashes during 20 turns
- ✅ **Memory:** Memory usage stable (no leaks)
- ✅ **Performance:** Response times consistent (< 10% degradation)
- ✅ **Completeness:** GET returns all 40 messages (20 user + 20 assistant)
- ✅ **Accuracy:** All messages have correct content
- ✅ **Order:** Chronological order maintained

### Metrics to Record

| Metric | Turn 1 | Turn 10 | Turn 20 | Trend |
|--------|--------|---------|---------|-------|
| Response time (ms) | | | | |
| Memory usage (MB) | | | | |
| MessageDB size (events) | | | | |
| GET request time (ms) | | | | |

### Warning Signs

🚨 **Stop test if:**
- Memory usage grows > 500MB
- Response time > 30 seconds
- Server becomes unresponsive
- Error rate > 10%

---

## Test 5: Real LLM Response Quality

**Objective:** Validate that responses from actual Claude/Gemini API are sensible and tool calls work correctly.

**Why manual:** Requires human judgment of response quality.

### Prerequisites

Set up GCP credentials:
```bash
gcloud auth application-default login
export GCP_PROJECT_ID=your-project-id
```

Modify `main.rs` to use real LLM provider instead of mock.

### Test Procedure

#### Test 5A: Simple Conversation Quality

1. Start server with real LLM configured

2. Send a variety of prompts:
   ```javascript
   const threadId = crypto.randomUUID();

   async function testPrompt(text) {
     const response = await fetch(`http://127.0.0.1:3030/api/v1/threads/${threadId}`, {
       method: 'POST',
       headers: { 'Content-Type': 'application/json' },
       body: JSON.stringify({ text })
     });

     // Read response
     // ... SSE consumption code ...
   }

   // Test various prompts
   await testPrompt("What is 2+2?");
   await testPrompt("Explain quantum computing in one sentence");
   await testPrompt("Write a haiku about coffee");
   ```

3. Evaluate each response:

| Prompt | Response | Quality (1-5) | Notes |
|--------|----------|---------------|-------|
| "What is 2+2?" | | ⭐⭐⭐⭐⭐ | Correct answer? |
| "Explain quantum..." | | ⭐⭐⭐⭐⭐ | Concise & accurate? |
| "Write a haiku..." | | ⭐⭐⭐⭐⭐ | Valid haiku format? |

**Success criteria:**
- All responses factually correct
- Responses appropriate length
- No hallucinations
- Context maintained across turns

#### Test 5B: Tool Calling Quality

1. Configure agent with real tools (e.g., calculator, weather API)

2. Send prompts requiring tool use:
   ```javascript
   await testPrompt("What is 157 * 234? Use the calculator.");
   await testPrompt("What's the weather in Tokyo?");
   ```

3. Verify:
   - ✅ LLM decides to use appropriate tool
   - ✅ Tool called with correct arguments
   - ✅ Tool result used in final response
   - ✅ Final response is coherent

4. Check SSE events:
   - `agent_text`: "Let me calculate..."
   - `tool_call`: calculator with {a: 157, b: 234}
   - `tool_response`: {result: 36738}
   - `agent_text`: "The result is 36,738"

**Quality checklist:**
- [ ] LLM chooses correct tool (not wrong tool or no tool)
- [ ] Arguments extracted correctly from user message
- [ ] Result integrated naturally into response
- [ ] No hallucinated tool calls

#### Test 5C: Multi-Step Reasoning

1. Ask a question requiring multiple tool calls:
   ```
   "What's the total cost if I buy 3 items at $15.99 each and 2 items
   at $24.50 each, with 8% sales tax? Show your work."
   ```

2. Observe agent behavior:
   - Does it break down the problem?
   - Does it make multiple tool calls if needed?
   - Is the final answer correct?

3. Check MessageDB for iteration count:
   ```bash
   # Connect to MessageDB
   psql postgresql://postgres:message_store_password@localhost:5433/message_store

   # Count iterations
   SELECT data->>'type', COUNT(*)
   FROM messages
   WHERE stream_name = 'thread:v0-{your-thread-id}'
   GROUP BY data->>'type';
   ```

**Success criteria:**
- ✅ Correct final answer
- ✅ Logical step-by-step reasoning
- ✅ Iterations < max_iterations
- ✅ All steps recorded in MessageDB

---

## Test 6: Error Recovery and Edge Cases

**Objective:** Verify system handles errors gracefully and provides useful feedback.

**Why manual:** Error scenarios require observation of user-facing behavior.

### Test Cases

#### Case A: MessageDB Unavailable

1. Stop MessageDB:
   ```bash
   docker-compose stop
   ```

2. Try to POST message:
   ```bash
   curl -X POST http://127.0.0.1:3030/api/v1/threads/test-123 \
     -H "Content-Type: application/json" \
     -d '{"text": "Hello"}'
   ```

**Expected behavior:**
- ❌ **Bad:** Server crashes or hangs
- ✅ **Good:** Returns 500 error with clear message
- ✅ **Best:** Returns SSE error event, connection closes gracefully

**Verify:**
- [ ] Error message is informative (mentions database connection)
- [ ] Server logs error but continues running
- [ ] Subsequent requests work after DB restored

#### Case B: LLM API Failure

1. Configure LLM provider to simulate API error (modify mock or break credentials)

2. POST message

**Expected behavior:**
- ✅ SSE stream includes `error` event
- ✅ Error message indicates LLM failure
- ✅ `AgentFailed` event written to MessageDB
- ✅ GET thread still works (shows partial conversation)

**Verify:**
- [ ] Error doesn't expose sensitive details (API keys, etc.)
- [ ] User gets actionable error message
- [ ] Server remains stable

#### Case C: Malformed Request

1. Send invalid JSON:
   ```bash
   curl -X POST http://127.0.0.1:3030/api/v1/threads/test-123 \
     -H "Content-Type: application/json" \
     -d 'invalid json{'
   ```

2. Send missing field:
   ```bash
   curl -X POST http://127.0.0.1:3030/api/v1/threads/test-123 \
     -H "Content-Type: application/json" \
     -d '{}'
   ```

**Expected behavior:**
- ✅ Returns 400 Bad Request
- ✅ Error message describes what's wrong
- ✅ Server doesn't crash

#### Case D: Invalid Thread ID

1. GET with invalid UUID format:
   ```bash
   curl http://127.0.0.1:3030/api/v1/threads/not-a-uuid
   ```

**Expected behavior:**
- ✅ Returns 400 or 404
- ✅ Error message clear
- ✅ No panic

#### Case E: Tool Execution Failure

1. Configure tool that always fails (e.g., network error)

2. POST message that triggers tool

**Expected behavior:**
- ✅ SSE includes `tool_response` with error
- ✅ `ToolExecutionFailed` event in MessageDB
- ✅ Agent handles gracefully (doesn't retry infinitely)
- ✅ Final response acknowledges failure

### Error Message Quality Rubric

Rate each error message:

| Aspect | 1 (Poor) | 3 (Good) | 5 (Excellent) |
|--------|----------|----------|---------------|
| **Clarity** | Cryptic error code | Generic message | Specific, understandable |
| **Actionability** | No guidance | Hints at issue | Clear next steps |
| **Security** | Leaks internals | Sanitized | No sensitive data |
| **Consistency** | Different formats | Mostly consistent | Uniform structure |

---

## Test 7: Docker Compose Infrastructure

**Objective:** Verify the full stack runs correctly from docker-compose setup.

**Why manual:** Infrastructure validation requires real environment.

### Test Procedure

1. Clean start:
   ```bash
   docker-compose down -v
   docker-compose up -d
   ```

2. Check MessageDB is running:
   ```bash
   docker-compose ps
   # Should show messagedb service running

   docker-compose logs messagedb
   # Should show PostgreSQL ready
   ```

3. Verify database accessibility:
   ```bash
   psql postgresql://postgres:message_store_password@localhost:5433/message_store -c "SELECT version();"
   ```

4. Start application:
   ```bash
   cargo run
   ```

5. Verify connection in server logs:
   ```
   ✓ Connected to MessageDB
   ```

6. Run basic POST/GET cycle (from Test 1)

7. Restart MessageDB:
   ```bash
   docker-compose restart messagedb
   ```

8. Verify server reconnects and continues working

### Success Criteria

- ✅ **Clean start:** `docker-compose up -d` works first time
- ✅ **Connectivity:** Application connects to MessageDB on port 5433
- ✅ **Persistence:** Data survives MessageDB restart
- ✅ **No conflicts:** Port 5433 not conflicting with default PostgreSQL (5432)
- ✅ **Logs:** No errors in docker-compose logs

### Troubleshooting Guide

| Problem | Solution |
|---------|----------|
| Port 5433 already in use | Change port in docker-compose.yml |
| Connection refused | Check firewall, verify container is running |
| "role postgres does not exist" | Database not initialized, check logs |
| Server can't connect | Check connection string in main.rs matches docker-compose |

---

## Test 8: Performance Baseline

**Objective:** Establish performance baselines for monitoring regression.

**Why manual:** Requires controlled environment and measurement.

### Metrics to Collect

1. **Latency (Time to First Byte)**
   ```bash
   time curl -X POST http://127.0.0.1:3030/api/v1/threads/test-perf \
     -H "Content-Type: application/json" \
     -d '{"text": "Hello"}' \
     -N  # Keep connection open for SSE
   ```

2. **Throughput (Concurrent Requests)**
   ```bash
   # Use Apache Bench or wrk
   ab -n 100 -c 10 -p message.json -T 'application/json' \
     http://127.0.0.1:3030/api/v1/threads/test-perf
   ```

3. **MessageDB Write Performance**
   - POST 100 messages
   - Measure total time
   - Calculate events/second

4. **Projection Performance (GET)**
   - Create thread with 100 events
   - Measure GET response time
   - Repeat for 200, 500, 1000 events
   - Plot degradation curve

### Baseline Targets

| Metric | Target | Actual | Pass? |
|--------|--------|--------|-------|
| Time to first SSE event | < 500ms | | |
| POST 99th percentile | < 2s | | |
| GET thread (100 events) | < 200ms | | |
| GET thread (1000 events) | < 1s | | |
| Concurrent threads (10) | All succeed | | |
| Memory usage (idle) | < 100MB | | |
| Memory usage (10 threads) | < 200MB | | |

### Recording Results

Create a performance baseline document:

```markdown
## Performance Baseline - [Date]

### Environment
- Hardware: [Mac M1, 16GB RAM]
- MessageDB: [Docker container, default config]
- LLM: [Mock provider]

### Results
- POST latency (median): 42ms
- POST latency (p99): 156ms
- GET latency (100 events): 23ms
- GET latency (1000 events): 187ms
- Throughput: 45 requests/second
- Max concurrent threads tested: 50

### Notes
- [Any observations]
```

Save as `docs/performance-baseline-YYYY-MM-DD.md`

---

## Reporting Issues

When you find an issue during manual testing:

### Issue Template

```markdown
## Manual Test Failure: [Brief Description]

**Test:** [Test name from this guide]
**Date:** [YYYY-MM-DD]
**Tester:** [Your name]

### Steps to Reproduce
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Expected Behavior
[What should happen]

### Actual Behavior
[What actually happened]

### Logs/Screenshots
[Paste relevant logs or attach screenshots]

### Environment
- OS: [macOS, Linux, Windows]
- Rust version: `rustc --version`
- Docker version: `docker --version`
- MessageDB version: [from docker-compose.yml]

### Severity
- [ ] Critical - Blocks release
- [ ] High - Should fix before release
- [ ] Medium - Should fix soon
- [ ] Low - Nice to have
```

---

## Manual Testing Checklist

Before each release, complete this checklist:

### Pre-Release Manual Testing

- [ ] Test 1: SSE Streaming UX (Chrome, Firefox)
- [ ] Test 2: Browser Compatibility (all browsers)
- [ ] Test 3: Network Interruption Recovery
- [ ] Test 4: Long-Running Conversation (20+ turns)
- [ ] Test 5: Real LLM Response Quality
  - [ ] 5A: Simple conversation
  - [ ] 5B: Tool calling
  - [ ] 5C: Multi-step reasoning
- [ ] Test 6: Error Recovery (all cases A-E)
- [ ] Test 7: Docker Compose Infrastructure
- [ ] Test 8: Performance Baseline

### Sign-Off

```
Tested by: ________________
Date: ________________
Version: ________________
Result: ☐ PASS  ☐ FAIL (see issues)
```

---

## Continuous Improvement

After each manual testing session:

1. **Document issues** found and fixed
2. **Update this guide** if procedures change
3. **Consider automation** - Can any manual test be automated?
4. **Review metrics** - Are performance baselines still valid?
5. **Update checklists** - Add new scenarios discovered

---

## Quick Reference

### Useful Commands

```bash
# Start everything
docker-compose up -d && cargo run

# Check MessageDB contents
psql postgresql://postgres:message_store_password@localhost:5433/message_store \
  -c "SELECT stream_name, type, data FROM messages ORDER BY global_position DESC LIMIT 10;"

# Monitor server resources
watch -n 2 'ps aux | grep rust2 | grep -v grep'

# Test SSE from command line
curl -N http://127.0.0.1:3030/api/v1/threads/test-123 \
  -H "Accept: text/event-stream"

# Pretty-print thread
curl http://127.0.0.1:3030/api/v1/threads/test-123 | jq .
```

### Common Thread IDs for Testing

Use consistent UUIDs for reproducible testing:

- `11111111-1111-1111-1111-111111111111` - Basic tests
- `22222222-2222-2222-2222-222222222222` - Tool use tests
- `33333333-3333-3333-3333-333333333333` - Error tests
- `44444444-4444-4444-4444-444444444444` - Performance tests

---

## Related Documents

- `plans/phase5-automated-tests.md` - Automated test suite specification
- `plans/event-sourced-agent.md` - Phase 5 implementation plan
- `README.md` - API documentation and usage
