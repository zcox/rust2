# Warp to Axum Migration Plan

## Executive Summary

Migrate the HTTP server from Warp to Axum to eliminate the `Send + Sync` trait requirement for SSE streams. Axum is the modern successor to Warp, built by the same Tokio team, and only requires `Send` for streams (not `Sync`), which aligns better with Rust's stream semantics.

## Motivation

### Current Problem
- Warp requires SSE streams to be `Send + Sync`
- `EventSourcedAgent.run()` returns streams that are `Send` but not `Sync`
- Current workaround uses tokio channels to make streams `Sync`, adding unnecessary complexity

### Why Axum?
- **Built by Tokio team** - Spiritual successor to Warp, better long-term support
- **Relaxed trait bounds** - Only requires `Send` for SSE streams (v0.7+)
- **Better ergonomics** - More intuitive API, better error messages
- **Ecosystem momentum** - Most new Rust web projects use Axum
- **Type safety** - Stronger compile-time guarantees via extractors
- **Active development** - Regular updates, larger community

### Benefits
1. Eliminate the tokio channel workaround in `EventSourcedAgent`
2. Cleaner, more maintainable code
3. Better alignment with Rust async ecosystem
4. Improved developer experience
5. Future-proof architecture

## Scope of Changes

### Files to Modify
1. `Cargo.toml` - Update dependencies
2. `src/main.rs` - Replace Warp server with Axum
3. `src/routes.rs` - Rewrite route definitions for Axum
4. `src/handlers/send_message.rs` - Update to Axum handler signature
5. `src/handlers/get_thread.rs` - Update to Axum handler signature
6. `src/sse.rs` - Adapt SSE helpers for Axum's Event type
7. `src/llm/agent/event_sourced.rs` - Remove `Sync` from return type (simplification!)

### Files Unaffected
- All Message DB code (`src/message_db/`)
- All LLM abstraction code (`src/llm/core/`, `src/llm/claude/`, `src/llm/gemini/`)
- Event sourcing logic (`src/llm/agent/events.rs`, `projection.rs`, `store.rs`)
- Model definitions (`src/models.rs`) - minor adjustments only
- All tests and examples (except integration tests that call HTTP endpoints)

## Migration Plan

### Phase 1: Dependencies

**File:** `Cargo.toml`

**Remove:**
```toml
warp = "0.3"
```

**Add:**
```toml
axum = "0.7"
tower = "0.4"           # Middleware support
tower-http = { version = "0.5", features = ["cors", "trace"] }  # CORS, logging
```

**Keep:**
```toml
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
futures = "0.3"
futures-util = "0.3"
# ... all other deps remain
```

### Phase 2: Main Server Entry Point

**File:** `src/main.rs`

**Current (Warp):**
```rust
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = create_routes(agent, store);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}
```

**New (Axum):**
```rust
use axum::{Router, Server};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (optional but recommended)
    tracing_subscriber::fmt::init();

    // Build shared state
    let app_state = AppState {
        agent: Arc::new(event_sourced_agent),
        store: Arc::new(thread_store),
    };

    // Create router
    let app = create_routes(app_state)
        .layer(TraceLayer::new_for_http());  // Request logging

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3030));
    println!("Server running on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
```

**Key Changes:**
- Use `Router` instead of `Filter`
- State management via `AppState` struct instead of `with()` filters
- `TraceLayer` for request logging (replaces manual logging)
- `Server::bind()` instead of `warp::serve()`

**New AppState struct:**
```rust
#[derive(Clone)]
struct AppState {
    agent: Arc<EventSourcedAgent>,
    store: Arc<ThreadStore>,
}
```

### Phase 3: Route Definitions

**File:** `src/routes.rs`

**Current (Warp):**
```rust
pub fn create_routes(
    agent: Arc<EventSourcedAgent>,
    store: Arc<ThreadStore>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let send_message = warp::path!("api" / "v1" / "threads" / Uuid)
        .and(warp::post())
        .and(warp::body::json())
        .and(with_agent(agent.clone()))
        .and_then(send_message_handler);

    let get_thread = warp::path!("api" / "v1" / "threads" / Uuid)
        .and(warp::get())
        .and(with_store(store.clone()))
        .and_then(get_thread_handler);

    send_message.or(get_thread)
}
```

**New (Axum):**
```rust
use axum::{
    routing::{get, post},
    Router,
};

pub fn create_routes(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/threads/:thread_id", post(send_message_handler))
        .route("/api/v1/threads/:thread_id", get(get_thread_handler))
        .with_state(state)
}
```

**Key Changes:**
- `Router::new()` instead of combining filters
- `.route()` method for each endpoint
- Path parameters use `:param` syntax instead of type-based extraction
- `.with_state()` for shared state instead of `.and(with_x())` filters
- No need for custom `with_agent()` and `with_store()` helpers

### Phase 4: POST Handler (SSE Streaming)

**File:** `src/handlers/send_message.rs`

**Current (Warp):**
```rust
pub async fn send_message_handler(
    thread_id: Uuid,
    request: SendMessageRequest,
    agent: Arc<EventSourcedAgent>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let agent_stream = agent.run(thread_id_str, request.text).await
        .map_err(|e| warp::reject::custom(AgentErrorRejection(e.to_string())))?;

    let sse_stream = agent_stream.map(|event_result| {
        // ... convert to SSE events
    });

    Ok(warp::sse::reply(
        warp::sse::keep_alive().stream(sse_stream),
    ))
}

#[derive(Debug)]
struct AgentErrorRejection(String);
impl warp::reject::Reject for AgentErrorRejection {}
```

**New (Axum):**
```rust
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use std::convert::Infallible;

pub async fn send_message_handler(
    Path(thread_id): Path<Uuid>,
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    println!("POST /threads/{}: {}", thread_id, request.text);

    // Run agent and get event stream
    let agent_stream = state.agent
        .run(thread_id.to_string(), request.text)
        .await
        .map_err(AppError::Agent)?;

    // Convert AgentEvent stream to SSE Event stream
    let sse_stream = agent_stream.map(|event_result| {
        match event_result {
            Ok(agent_event) => match agent_event {
                AgentEvent::TextDelta(text) => {
                    Ok(Event::default()
                        .event("agent_text")
                        .json_data(json!({ "text": text }))
                        .unwrap())
                }
                AgentEvent::ToolExecutionStarted { tool_use_id, name, input } => {
                    Ok(Event::default()
                        .event("tool_call")
                        .json_data(json!({
                            "id": tool_use_id,
                            "name": name,
                            "input": input
                        }))
                        .unwrap())
                }
                AgentEvent::ToolExecutionCompleted { tool_use_id, name, result } => {
                    let result_value = serde_json::from_str(&result)
                        .unwrap_or_else(|_| json!({ "result": result }));
                    Ok(Event::default()
                        .event("tool_response")
                        .json_data(json!({
                            "id": format!("response-{}", tool_use_id),
                            "tool_use_id": tool_use_id,
                            "result": result_value
                        }))
                        .unwrap())
                }
                AgentEvent::Completed => {
                    Ok(Event::default()
                        .event("done")
                        .data(""))
                }
                _ => Ok(Event::default().data("")),  // Ignore other events
            },
            Err(e) => {
                eprintln!("Stream error: {:?}", e);
                Ok(Event::default()
                    .event("error")
                    .json_data(json!({ "error": e.to_string() }))
                    .unwrap())
            }
        }
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}
```

**Key Changes:**
- `Path(thread_id): Path<Uuid>` extractor instead of function parameter
- `State(state): State<AppState>` for shared state
- `Json(request): Json<SendMessageRequest>` for request body
- Return `Result<Sse<impl Stream>, AppError>` instead of `Result<impl Reply, Rejection>`
- `Event::default()` for creating SSE events
- `.json_data()` method for serializing JSON payloads
- `Infallible` error type (Axum convention for streams that don't error)
- No custom rejection type needed - use custom `AppError` enum

### Phase 5: GET Handler (JSON Response)

**File:** `src/handlers/get_thread.rs`

**Current (Warp):**
```rust
pub async fn get_thread_handler(
    thread_id: Uuid,
    store: Arc<ThreadStore>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let events = store.read_thread_events(&thread_id.to_string())
        .await
        .map_err(|e| warp::reject::custom(StoreErrorRejection(e.to_string())))?;

    let messages = project_events_to_messages(&events);

    let thread = Thread {
        id: thread_id,
        messages,
        // ... metadata
    };

    Ok(warp::reply::json(&thread))
}
```

**New (Axum):**
```rust
use axum::{
    extract::{Path, State},
    Json,
};

pub async fn get_thread_handler(
    Path(thread_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Thread>, AppError> {
    println!("GET /threads/{}", thread_id);

    // Read thread events
    let events = state.store
        .read_thread_events(&thread_id.to_string())
        .await
        .map_err(AppError::Store)?;

    // Project to messages
    let messages = project_events_to_messages(&events);

    // Build response
    let thread = Thread {
        id: thread_id,
        messages,
        created_at: Utc::now(),  // TODO: Extract from first event
        updated_at: Utc::now(),  // TODO: Extract from last event
    };

    Ok(Json(thread))
}
```

**Key Changes:**
- `Path(thread_id): Path<Uuid>` extractor
- `State(state): State<AppState>` for accessing store
- Return `Result<Json<Thread>, AppError>` (Axum auto-serializes)
- No `warp::reply::json()` wrapper needed

### Phase 6: Error Handling

**New File:** `src/error.rs` (or add to existing file)

**Axum Error Type:**
```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Agent(AgentError),
    Store(String),
    NotFound,
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Agent(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Agent error: {}", e),
            ),
            AppError::Store(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Store error: {}", e),
            ),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "Thread not found".to_string(),
            ),
            AppError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error: {}", e),
            ),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

impl From<AgentError> for AppError {
    fn from(e: AgentError) -> Self {
        AppError::Agent(e)
    }
}
```

**Key Changes:**
- Replace Warp's `Rejection` system with `IntoResponse` trait
- Centralized error handling
- Automatic JSON serialization of errors
- Custom status codes per error type

### Phase 7: SSE Helper Updates

**File:** `src/sse.rs`

**Current (Warp):**
```rust
pub fn create_agent_text_event(thread_id: String, text: String) -> Result<Event, warp::Error> {
    Ok(Event::default()
        .event("agent_text")
        .data(json!({ "threadId": thread_id, "text": text }).to_string()))
}

pub fn create_tool_call_event(...) -> Result<Event, warp::Error> { ... }
pub fn create_tool_response_event(...) -> Result<Event, warp::Error> { ... }
pub fn create_done_event() -> Result<Event, warp::Error> { ... }
```

**New (Axum):**
```rust
use axum::response::sse::Event;

pub fn create_agent_text_event(thread_id: String, text: String) -> Event {
    Event::default()
        .event("agent_text")
        .json_data(json!({ "threadId": thread_id, "text": text }))
        .expect("Failed to serialize agent_text event")
}

pub fn create_tool_call_event(
    id: String,
    name: String,
    input: serde_json::Value,
) -> Event {
    Event::default()
        .event("tool_call")
        .json_data(json!({
            "id": id,
            "name": name,
            "input": input
        }))
        .expect("Failed to serialize tool_call event")
}

pub fn create_tool_response_event(
    id: String,
    tool_use_id: String,
    result: serde_json::Value,
) -> Event {
    Event::default()
        .event("tool_response")
        .json_data(json!({
            "id": id,
            "tool_use_id": tool_use_id,
            "result": result
        }))
        .expect("Failed to serialize tool_response event")
}

pub fn create_done_event() -> Event {
    Event::default()
        .event("done")
        .data("")
}
```

**Key Changes:**
- Return `Event` directly instead of `Result<Event, warp::Error>`
- Use `.json_data()` instead of manual `.to_string()`
- `.expect()` for serialization errors (should never fail with our types)
- Simpler API - no error handling needed at call sites

### Phase 8: EventSourcedAgent Simplification

**File:** `src/llm/agent/event_sourced.rs`

**Current:**
```rust
pub async fn run(
    &self,
    thread_id: String,
    user_message: String,
) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send + Sync>>, AgentError>
{
    // Uses tokio channel workaround to make stream Sync
    let (tx, rx) = mpsc::channel(100);
    tokio::spawn(async move { ... });
    Ok(Box::pin(ReceiverStream::new(rx)))
}
```

**New (Simplified):**
```rust
pub async fn run(
    &self,
    thread_id: String,
    user_message: String,
) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send>>, AgentError>
//                                                                           ^^^^ Removed Sync!
{
    // Option A: Direct stream (if we can make it work without channels)
    // Option B: Keep channel approach but know it's for buffering, not Sync workaround

    // For now, keep the channel implementation as-is, just update the signature
    let (tx, rx) = mpsc::channel(100);
    tokio::spawn(async move { ... });
    Ok(Box::pin(ReceiverStream::new(rx)))
}
```

**Key Changes:**
- Remove `+ Sync` bound from return type
- Document that channel is for clean separation, not a workaround
- Optionally explore removing channel entirely (future optimization)

## Testing Strategy

### Unit Tests
- No changes needed - all unit tests are HTTP-framework-agnostic
- Event projection, store operations, LLM mapping all remain the same

### Integration Tests
**Files to update:**
- Any tests that make HTTP requests to the server
- Update from `warp::test` utilities to Axum test utilities

**Before (Warp):**
```rust
#[tokio::test]
async fn test_send_message() {
    let routes = create_routes(agent, store);
    let resp = warp::test::request()
        .method("POST")
        .path("/api/v1/threads/550e8400-e29b-41d4-a716-446655440000")
        .json(&SendMessageRequest { text: "Hello".into() })
        .reply(&routes)
        .await;

    assert_eq!(resp.status(), 200);
}
```

**After (Axum):**
```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;  // for `oneshot`

#[tokio::test]
async fn test_send_message() {
    let app = create_routes(app_state);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/threads/550e8400-e29b-41d4-a716-446655440000")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&SendMessageRequest { text: "Hello".into() }).unwrap()
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

### Manual Testing
1. Start server: `cargo run`
2. Test POST endpoint:
   ```bash
   curl -N -H "Accept: text/event-stream" \
     -H "Content-Type: application/json" \
     -d '{"text":"What is 2+2?"}' \
     http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
   ```
3. Test GET endpoint:
   ```bash
   curl http://localhost:3030/api/v1/threads/550e8400-e29b-41d4-a716-446655440000
   ```
4. Verify SSE events stream correctly
5. Verify thread history retrieval works

## Migration Checklist

- [ ] **Phase 1: Dependencies**
  - [ ] Update `Cargo.toml` with Axum dependencies
  - [ ] Remove Warp dependency
  - [ ] Run `cargo check` to verify dependencies resolve

- [ ] **Phase 2: Main Server**
  - [ ] Create `AppState` struct
  - [ ] Replace Warp server initialization with Axum
  - [ ] Add tracing/logging setup (optional)
  - [ ] Test: `cargo build` succeeds

- [ ] **Phase 3: Routes**
  - [ ] Rewrite `src/routes.rs` with Axum `Router`
  - [ ] Remove `with_agent()` and `with_store()` filters
  - [ ] Test: `cargo build` succeeds

- [ ] **Phase 4: Error Handling**
  - [ ] Create `AppError` type with `IntoResponse` impl
  - [ ] Remove custom rejection types
  - [ ] Test: Error responses serialize correctly

- [ ] **Phase 5: POST Handler**
  - [ ] Update `send_message_handler` signature
  - [ ] Convert to Axum extractors (`Path`, `State`, `Json`)
  - [ ] Update SSE stream construction
  - [ ] Test: SSE streaming works with curl

- [ ] **Phase 6: GET Handler**
  - [ ] Update `get_thread_handler` signature
  - [ ] Convert to Axum extractors
  - [ ] Return `Json<Thread>` directly
  - [ ] Test: JSON response works with curl

- [ ] **Phase 7: SSE Helpers**
  - [ ] Update `src/sse.rs` to use Axum's `Event` type
  - [ ] Switch to `.json_data()` method
  - [ ] Remove `Result` wrappers (return `Event` directly)
  - [ ] Test: Events serialize correctly

- [ ] **Phase 8: EventSourcedAgent**
  - [ ] Remove `+ Sync` from return type signature
  - [ ] Update documentation
  - [ ] Consider removing channel workaround (optional)
  - [ ] Test: Agent streams work with Axum

- [ ] **Phase 9: Integration Tests**
  - [ ] Update HTTP tests to use `tower::ServiceExt::oneshot`
  - [ ] Update request construction
  - [ ] Verify all tests pass

- [ ] **Phase 10: Documentation**
  - [ ] Update README.md with Axum examples
  - [ ] Update CLAUDE.md architecture notes
  - [ ] Update API documentation if needed

- [ ] **Phase 11: Final Validation**
  - [ ] All unit tests pass: `cargo test`
  - [ ] All integration tests pass
  - [ ] Manual testing: POST with SSE streaming works
  - [ ] Manual testing: GET returns thread history
  - [ ] Manual testing: Error cases handled correctly
  - [ ] Code review: Remove all Warp imports

## Rollback Plan

If migration encounters issues:

1. **Keep Warp in parallel** (feature flag approach):
   ```toml
   [dependencies]
   warp = { version = "0.3", optional = true }
   axum = { version = "0.7", optional = true }

   [features]
   default = ["warp-server"]
   warp-server = ["warp"]
   axum-server = ["axum", "tower", "tower-http"]
   ```

2. **Maintain both implementations** until Axum is stable:
   - `src/server/warp.rs` - Original Warp code
   - `src/server/axum.rs` - New Axum code
   - `src/main.rs` - Feature flag to choose server

3. **Incremental rollout**:
   - Deploy Axum version to staging first
   - Monitor for issues
   - Rollback to Warp if critical bugs found

## Timeline Estimate

- **Phase 1-2 (Dependencies + Main):** 30 minutes
- **Phase 3-4 (Routes + Errors):** 30 minutes
- **Phase 5-6 (Handlers):** 1 hour
- **Phase 7 (SSE Helpers):** 15 minutes
- **Phase 8 (EventSourcedAgent):** 15 minutes
- **Phase 9 (Tests):** 1 hour
- **Phase 10-11 (Docs + Validation):** 30 minutes

**Total: ~4 hours** (for experienced Rust developer)

## Post-Migration Opportunities

Once on Axum, we can leverage:

1. **Middleware ecosystem**:
   - Request tracing with `tower-http`
   - CORS handling
   - Compression
   - Rate limiting

2. **Better extractors**:
   - Custom extractors for auth
   - Validation extractors
   - Type-safe query parameters

3. **WebSocket support**:
   - Future enhancement: WebSockets instead of SSE
   - Built-in WebSocket support in Axum

4. **Performance**:
   - Benchmark Axum vs Warp (likely similar, both use Hyper)
   - Optimize based on real-world usage

## References

- [Axum Documentation](https://docs.rs/axum/latest/axum/)
- [Axum GitHub Repository](https://github.com/tokio-rs/axum)
- [Migrating from Warp to Axum (community guide)](https://github.com/tokio-rs/axum/discussions/1670)
- [Axum SSE Example](https://github.com/tokio-rs/axum/blob/main/examples/sse/src/main.rs)
- [Tower Middleware](https://docs.rs/tower/latest/tower/)

## Open Questions

1. **CORS configuration**: Do we need CORS? If so, configure `tower-http::cors`
2. **Authentication**: Future auth middleware strategy?
3. **Rate limiting**: Should we add rate limiting at HTTP layer?
4. **Metrics**: Should we add Prometheus metrics during migration?
5. **Health checks**: Add `/health` endpoint while we're refactoring?

## Success Criteria

- [ ] All existing functionality works identically
- [ ] SSE streaming delivers same events in same format
- [ ] GET /threads/{id} returns same JSON structure
- [ ] No performance regression (benchmark)
- [ ] All tests pass
- [ ] Code is cleaner and more maintainable
- [ ] No Warp dependencies remain
- [ ] Documentation updated
