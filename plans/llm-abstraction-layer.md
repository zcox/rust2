# LLM Abstraction Layer for Vertex AI (Gemini & Claude)

## 1. Executive Overview

This plan outlines the implementation of a production-grade Rust library providing a unified interface for interacting with Anthropic Claude and Google Gemini models hosted on Google Cloud Platform's Vertex AI. The library will implement custom HTTP clients for both providers, bypassing existing SDKs to achieve precise control over streaming, authentication, and tool use.

### Key Requirements
- **Models Supported**:
  - Gemini: 2.5 Pro, 2.5 Flash, 2.5 Flash Lite
  - Claude: Sonnet 4.5, Haiku 4.5
- **Core Features**: Token streaming, tool calling (including parallel), tool result handling
- **Authentication**: Application Default Credentials (ADC) via existing library
- **Architecture**: Common abstraction layer with provider-specific implementations

## 2. Core Abstraction Layer

The abstraction layer provides a unified interface that both Gemini and Claude implementations will satisfy. This enables application code to be provider-agnostic.

### 2.1 Primary Types

#### `LlmProvider` Trait
The main interface that both implementations will satisfy:

```rust
#[async_trait]
pub trait LlmProvider {
    async fn stream_generate(
        &self,
        request: GenerateRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent>>>>, LlmError>;
}
```

#### `GenerateRequest`
Common request structure:
- `messages: Vec<Message>` - Conversation history
- `tools: Option<Vec<ToolDeclaration>>` - Available tools
- `config: GenerationConfig` - Parameters (temperature, max_tokens, etc.)
- `system: Option<String>` - System prompt/instructions

#### `Message`
Represents a single message in the conversation:
- `role: MessageRole` - User, Assistant, or Tool
- `content: Vec<ContentBlock>` - Message content blocks

#### `MessageRole` Enum
- `User` - Human input
- `Assistant` - Model output
- `Tool` - Tool execution result

#### `ContentBlock` Enum
- `Text { text: String }` - Plain text content
- `ToolUse { id: String, name: String, input: serde_json::Value }` - Tool invocation
- `ToolResult { tool_use_id: String, content: String, is_error: bool }` - Tool execution result

#### `ToolDeclaration`
Defines a tool available to the model:
- `name: String` - Function name
- `description: String` - What the tool does
- `input_schema: serde_json::Value` - JSON Schema for parameters

#### `GenerationConfig`
Generation parameters:
- `max_tokens: u32` - Maximum output tokens
- `temperature: Option<f32>` - Randomness (0.0-1.0)
- `top_p: Option<f32>` - Nucleus sampling
- `top_k: Option<u32>` - Top-k sampling (Gemini-specific, ignored for Claude)
- `stop_sequences: Option<Vec<String>>` - Stop generation triggers

#### `StreamEvent` Enum
Events emitted during streaming:
- `ContentBlockStart { index: usize, block: ContentBlockStart }` - New content block begins
- `ContentDelta { index: usize, delta: ContentDelta }` - Incremental content
- `ContentBlockEnd { index: usize }` - Content block complete
- `MessageStart { message: MessageMetadata }` - Response begins
- `MessageDelta { usage: Option<UsageMetadata> }` - Message metadata update
- `MessageEnd { finish_reason: FinishReason, usage: UsageMetadata }` - Response complete
- `Error { error: String }` - Error occurred

#### `ContentDelta` Enum
- `TextDelta { text: String }` - Text token(s)
- `ToolUseDelta { partial: PartialToolUse }` - Partial tool call data

#### `FinishReason` Enum
- `EndTurn` / `Stop` - Natural completion
- `MaxTokens` - Hit token limit
- `StopSequence` - Hit stop sequence
- `ToolUse` - Waiting for tool execution
- `Safety` - Blocked by safety filters
- `Other(String)` - Provider-specific reason

#### `UsageMetadata`
Token usage information:
- `input_tokens: u32` - Prompt tokens consumed
- `output_tokens: u32` - Response tokens generated
- `total_tokens: u32` - Sum of input and output

### 2.2 Error Handling

#### `LlmError` Enum
- `AuthenticationError(String)` - ADC/token issues
- `HttpError { status: u16, body: String }` - HTTP failures
- `StreamError(String)` - SSE parsing failures
- `SerializationError(String)` - JSON encoding/decoding issues
- `InvalidRequest(String)` - Bad request parameters
- `RateLimitExceeded { retry_after: Option<Duration> }` - 429 responses
- `ProviderError { code: String, message: String }` - Provider-specific errors

## 3. Authentication Layer

### 3.1 ADC Integration
- Use `gcp_auth` crate (version 0.10+)
- Single `AuthenticationManager` instance shared across providers
- Token scope: `https://www.googleapis.com/auth/cloud-platform`
- Automatic token caching and refresh
- Support for:
  - `GOOGLE_APPLICATION_CREDENTIALS` environment variable
  - User credentials from `gcloud auth application-default login`
  - Metadata server (Compute Engine, Cloud Run, GKE)

### 3.2 Token Management
- Initialize `AuthenticationManager` once during client construction
- Reuse across all requests to leverage internal caching
- Handle token expiration transparently via `get_token()` before each request
- Inject token as `Authorization: Bearer {token}` header

## 4. Gemini Implementation

### 4.1 Endpoint Configuration
- Base URL: `https://{location}-aiplatform.googleapis.com/v1`
- Resource path: `/projects/{project_id}/locations/{location}/publishers/google/models/{model}:streamGenerateContent`
- Support `?alt=sse` query parameter for Server-Sent Events
- Default location: `us-central1` (configurable)
- Model identifiers:
  - `gemini-2.5-pro`
  - `gemini-2.5-flash`
  - `gemini-2.5-flash-lite`

### 4.2 Request Schema Mapping

**Abstraction → Gemini Mapping:**

`GenerateRequest` maps to Gemini's `GenerateContentRequest`:
- `messages` → `contents: [{ role, parts: [{ text }] }]`
  - Gemini uses "user" and "model" roles (not "assistant")
  - Tool results go in `parts` with `functionResponse` structure
- `tools` → `tools: [{ functionDeclarations: [...] }]`
- `config.max_tokens` → `generationConfig.maxOutputTokens`
- `config.temperature` → `generationConfig.temperature`
- `config.top_p` → `generationConfig.topP`
- `config.top_k` → `generationConfig.topK`
- `config.stop_sequences` → `generationConfig.stopSequences`
- `system` → `systemInstruction.parts[0].text`

**Tool Declaration Mapping:**
- `ToolDeclaration.name` → `functionDeclarations[].name`
- `ToolDeclaration.description` → `functionDeclarations[].description`
- `ToolDeclaration.input_schema` → `functionDeclarations[].parameters`

**Tool Use in Messages:**
- `ContentBlock::ToolUse` → `parts[].functionCall: { name, args }`
- `ContentBlock::ToolResult` → `parts[].functionResponse: { name, response }`

### 4.3 Response Schema Mapping

**Gemini → Abstraction Mapping:**

Gemini SSE stream format:
```
data: { "candidates": [{ "content": { "parts": [...], "role": "model" }, "finishReason": "...", "safetyRatings": [...] }], "usageMetadata": {...} }
```

- Parse each SSE `data:` line as JSON
- `candidates[0].content.parts[]` → iterate to emit `StreamEvent`s
  - `{ text }` → `ContentDelta::TextDelta`
  - `{ functionCall }` → `ContentDelta::ToolUseDelta`
- `finishReason` → map to `FinishReason`
  - `"STOP"` → `FinishReason::Stop`
  - `"MAX_TOKENS"` → `FinishReason::MaxTokens`
  - `"SAFETY"` → `FinishReason::Safety`
  - `"RECITATION"` → `FinishReason::Other("Recitation")`
- `usageMetadata` → emit in `MessageEnd` event
  - `promptTokenCount` → `input_tokens`
  - `candidatesTokenCount` → `output_tokens`

### 4.4 SSE Parsing Strategy
- Append `?alt=sse` to URL to get line-delimited events
- Read response body line-by-line using `BufReader` pattern
- Filter lines starting with `data:`
- Extract JSON payload after `data: `
- Deserialize to `GenerateContentResponse` struct
- Transform to abstraction's `StreamEvent`

### 4.5 Tool Call Handling
- Gemini may return multiple `functionCall` parts in a single response
- Each `functionCall` needs unique ID (generate UUID if not provided)
- Arguments are provided as complete JSON object (not streamed incrementally)
- For parallel tool calls: emit multiple `ToolUse` blocks in sequence

## 5. Claude Implementation

### 5.1 Endpoint Configuration
- Base URL: `https://{location}-aiplatform.googleapis.com/v1`
- Resource path: `/projects/{project_id}/locations/{location}/publishers/anthropic/models/{model}:streamRawPredict`
- No query parameters needed (SSE is default for `streamRawPredict`)
- Default location: `us-central1` (configurable)
- Model identifiers:
  - Sonnet 4.5: `claude-sonnet-4-5@20250929`
  - Haiku 4.5: `claude-haiku-4-5@20251001`

### 5.2 Request Schema Mapping

**Abstraction → Claude Mapping:**

`GenerateRequest` maps to Claude's `StreamRawPredictRequest`:
- **Critical**: `anthropic_version: "vertex-2023-10-16"` (required field)
- `messages` → `messages: [{ role, content }]`
  - Claude uses "user" and "assistant" roles
  - Content can be string or array of content blocks
  - Tool results use `{ type: "tool_result", tool_use_id, content }` format
- `tools` → `tools: [{ name, description, input_schema }]` (direct mapping)
- `config.max_tokens` → `max_tokens` (required, no default)
- `config.temperature` → `temperature`
- `config.top_p` → `top_p`
- `config.top_k` → **ignored** (Claude doesn't support)
- `config.stop_sequences` → `stop_sequences`
- `system` → `system` (top-level field)
- `stream: true` (always true for our use case)

**Tool Declaration Mapping:**
- Direct 1:1 mapping (Claude's schema matches our abstraction)

**Tool Use in Messages:**
- `ContentBlock::ToolUse` → `{ type: "tool_use", id, name, input }`
- `ContentBlock::ToolResult` → `{ type: "tool_result", tool_use_id, content }`

### 5.3 Response Schema Mapping

**Claude → Abstraction Mapping:**

Claude SSE stream format:
```
event: message_start
data: { "type": "message_start", "message": { "id": "...", "role": "assistant" } }

event: content_block_start
data: { "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }

event: content_block_delta
data: { "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Hello" } }

event: message_delta
data: { "type": "message_delta", "usage": { "output_tokens": 15 } }

event: message_stop
data: { "type": "message_stop" }
```

Event type mapping:
- `message_start` → `StreamEvent::MessageStart`
- `content_block_start` → `StreamEvent::ContentBlockStart`
- `content_block_delta` → `StreamEvent::ContentDelta`
  - `delta.type == "text_delta"` → `ContentDelta::TextDelta`
  - `delta.type == "input_json_delta"` → accumulate for `ContentDelta::ToolUseDelta`
- `content_block_stop` → `StreamEvent::ContentBlockEnd`
- `message_delta` → `StreamEvent::MessageDelta` (for usage updates)
- `message_stop` → `StreamEvent::MessageEnd`
- `error` → `StreamEvent::Error`

Finish reason mapping (from `message_delta.delta.stop_reason`):
- `"end_turn"` → `FinishReason::EndTurn`
- `"max_tokens"` → `FinishReason::MaxTokens`
- `"stop_sequence"` → `FinishReason::StopSequence`
- `"tool_use"` → `FinishReason::ToolUse`

Usage metadata:
- Accumulate from `message_start.message.usage` and `message_delta.usage`
- `input_tokens` → `input_tokens`
- `output_tokens` → `output_tokens`

### 5.4 SSE Parsing Strategy
- Read response body as byte stream
- Implement stateful parser:
  - Buffer incoming bytes
  - Scan for `\n\n` delimiter (event boundary)
  - Extract event lines
  - Parse `event:` line to determine type
  - Parse `data:` line as JSON
- Deserialize to Claude's event enum
- Transform to abstraction's `StreamEvent`

### 5.5 Tool Call Handling
- Claude streams tool call arguments incrementally via `input_json_delta`
- Must accumulate `partial_json` fragments until `content_block_stop`
- Only parse complete JSON after block ends
- Generate events:
  - `ContentBlockStart` when tool use begins
  - `ContentDelta::ToolUseDelta` for each delta (with accumulated partial state)
  - `ContentBlockEnd` when tool use complete (with fully parsed arguments)
- Parallel tool calls: Claude emits multiple content blocks sequentially (index 0, 1, 2...)

## 6. Key Differences Between Providers

### 6.1 Endpoint Structure
| Aspect | Gemini | Claude |
|--------|--------|--------|
| Method | `:streamGenerateContent` | `:streamRawPredict` |
| SSE Trigger | `?alt=sse` query param | Always SSE |
| Publisher | `/publishers/google/models/` | `/publishers/anthropic/models/` |

### 6.2 Request Payload
| Aspect | Gemini | Claude |
|--------|--------|--------|
| Role names | "user", "model" | "user", "assistant" |
| System prompt | `systemInstruction` object | `system` string |
| Required version | None | `anthropic_version: "vertex-2023-10-16"` |
| Tool schema | Nested in `tools[].functionDeclarations` | Direct `tools[]` array |
| Max tokens | Optional (`maxOutputTokens`) | Required (`max_tokens`) |
| Top-K | Supported | **Not supported** (ignore) |

### 6.3 Response Format
| Aspect | Gemini | Claude |
|--------|--------|--------|
| Event structure | Single `data:` JSON object | Multiple event types with `event:` + `data:` |
| Tool arguments | Complete JSON object | Streamed incrementally (must accumulate) |
| Usage metadata | In most chunks | Only in final `message_delta` |
| Finish reason | `finishReason` in candidate | `stop_reason` in message delta |

### 6.4 Handling Strategy
- **Abstraction layer normalizes** these differences
- **Provider implementations** handle specific wire formats
- **Application code** only sees unified `StreamEvent` stream
- **Configuration validation**: reject `top_k` for Claude (or silently ignore with warning)

## 7. Tool Execution Framework

### 7.1 Tool Executor Trait

```rust
#[async_trait]
pub trait ToolExecutor {
    async fn execute(
        &self,
        tool_use_id: String,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<String, String>; // Ok(result) or Err(error_message)
}
```

### 7.2 Function Registry Implementation

**Design:**
- Maintain `HashMap<String, Box<dyn Fn(serde_json::Value) -> Result<String, String>>>`
- Allow registration of Rust functions by name
- Deserialize arguments using `serde_json::from_value`
- Call function and serialize result back to string

**Implementation:**
```rust
pub struct FunctionRegistry {
    functions: HashMap<String, Box<dyn Fn(serde_json::Value) -> BoxFuture<'static, Result<String, String>> + Send + Sync>>,
}

impl FunctionRegistry {
    pub fn register<F, Args, R>(&mut self, name: &str, func: F)
    where
        F: Fn(Args) -> R + Send + Sync + 'static,
        Args: DeserializeOwned,
        R: Serialize,
    {
        // Wrapper that handles deserialization/serialization
    }
}
```

**Usage Pattern:**
```rust
let mut registry = FunctionRegistry::new();
registry.register("get_weather", |args: WeatherArgs| async {
    // implementation
    Ok(serde_json::to_string(&result)?)
});

let executor = FunctionRegistryExecutor::new(registry);
```

### 7.3 Argument Handling
- Arguments arrive as `serde_json::Value`
- Deserialize to concrete type using `serde::Deserialize`
- If deserialization fails, return error (not panic)
- Result serialization back to string (JSON)

### 7.4 Error Propagation
- If tool execution fails, create `ContentBlock::ToolResult` with `is_error: true`
- Error message goes in `content` field
- Model should handle gracefully and potentially retry or inform user

## 8. HTTP Client Implementation

### 8.1 Shared Infrastructure
- Use `reqwest` with features: `["json", "stream", "rustls-tls"]`
- `rustls-tls` for pure-Rust TLS (container-friendly, no OpenSSL dependency)
- Single `reqwest::Client` instance per provider (connection pooling)
- Configure timeouts:
  - `connect_timeout`: 5 seconds
  - No overall timeout (streaming can be long-running)

### 8.2 Request Construction
- Set `Authorization: Bearer {token}` header
- Set `Content-Type: application/json` header
- Serialize request body using `serde_json`
- POST to constructed endpoint URL

### 8.3 Error Handling
- Check HTTP status before parsing stream
- Non-2xx responses: read body as error JSON
- Parse provider-specific error format
- Map to `LlmError::ProviderError` or specific error types
- Handle 429 (rate limit) specially: extract `Retry-After` header

### 8.4 Retry Strategy
- Implement exponential backoff for transient errors (500, 503, 429)
- Max retries: 3
- Base delay: 1 second, exponential multiplier: 2
- Do not retry client errors (4xx except 429)
- Use `tokio::time::sleep` for delays

## 9. Testing Strategy

### 9.1 Unit Tests

**Authentication Layer:**
- Mock `gcp_auth` responses (if possible) or integration test only
- Test token caching behavior
- Test token refresh on expiry

**Request Mapping:**
- Test abstraction → Gemini JSON serialization
- Test abstraction → Claude JSON serialization
- Verify required fields present
- Verify optional fields omitted when None
- Test tool declaration conversion
- Test message role mapping

**Response Parsing:**
- Test Gemini SSE parsing with sample event streams
- Test Claude SSE parsing with sample event streams
- Test tool call accumulation (especially Claude's incremental args)
- Test error event handling
- Test usage metadata extraction

**Tool Execution:**
- Test function registry registration
- Test argument deserialization
- Test result serialization
- Test error handling in tool execution

**SSE Parser:**
- Test chunked data handling (event split across buffers)
- Test malformed events
- Test empty events
- Test large payloads

### 9.2 Integration Tests

**Setup:**
- Assume `gcloud auth application-default login` has been run (for ADC)
- Library clients accept `project_id` and `location` as direct constructor parameters
- Tests load configuration from a local `.env` file (not committed to git)
- Create `.env` in project root with:
  ```
  GCP_PROJECT_ID=your-project-id
  GCP_LOCATION=us-central1
  ```
- Provide a `.env.example` template for developers to copy
- Ensure `.env` is in `.gitignore`
- Skip tests if credentials not available (use `#[ignore]` attribute)

**Note:** The library itself has no opinion on where `project_id` and `location` come from - they're just configuration parameters like `model` or `temperature`. Applications may source them from env vars, config files, CLI args, or hardcode them. The test suite uses `.env` for convenience.

**Test Cases:**

*Gemini Tests:*
- Simple text generation (Gemini 2.5 Flash)
- Streaming with token count verification
- Tool call (single function)
- Parallel tool calls (multiple functions)
- Tool use → tool result → continuation
- Max tokens limit (verify finish reason)
- Temperature variation (behavior observation)

*Claude Tests:*
- Simple text generation (Claude Haiku 4.5)
- Streaming with token count verification
- Tool call (single function)
- Parallel tool calls (multiple functions)
- Tool use → tool result → continuation
- Max tokens limit (verify finish reason)
- System prompt handling

*Cross-Provider Tests:*
- Same prompt to both providers (verify abstraction works)
- Same tool definitions to both (verify portability)

**Example Integration Test Structure:**
```rust
#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_gemini_simple_generation() {
    // Load test config from .env file - applications can source these however they want
    dotenvy::dotenv().ok(); // Load .env, ignore if missing

    let project_id = env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required in .env");
    let location = env::var("GCP_LOCATION").unwrap_or_else(|_| "us-central1".to_string());

    let client = GeminiClient::new(project_id, location).await.unwrap();

    let request = GenerateRequest {
        messages: vec![Message::user("What is 2+2?")],
        tools: None,
        config: GenerationConfig::default(),
        system: None,
    };

    let mut stream = client.stream_generate(request).await.unwrap();
    let mut text = String::new();

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::ContentDelta { delta: ContentDelta::TextDelta { text: t }, .. } => {
                text.push_str(&t);
            }
            StreamEvent::MessageEnd { usage, .. } => {
                assert!(usage.total_tokens > 0);
            }
            _ => {}
        }
    }

    assert!(text.contains("4"));
}
```

### 9.3 Test Utilities
- Sample event builders for unit tests
- Mock SSE stream generator
- Fixture files with example Gemini/Claude responses
- Helper to create test tool declarations
- Helper to create test messages with tool results

## 10. Project Structure

**Note:** This repo contains multiple libraries. The LLM abstraction layer lives under `llm/` subdirectories alongside other libraries like `message_db`.

```
rust2/
├── Cargo.toml
├── .gitignore                # Include .env
├── .env.example              # Template for local .env (committed)
├── .env                      # Local config (NOT committed, created by developers)
├── src/
│   ├── lib.rs                 # Top-level exports (message_db, llm, etc.)
│   ├── message_db/            # Existing library
│   │   └── ...
│   └── llm/                   # LLM abstraction layer
│       ├── mod.rs             # LLM module exports
│       ├── core/
│       │   ├── mod.rs
│       │   ├── types.rs       # Core abstractions (Message, ContentBlock, etc.)
│       │   ├── provider.rs    # LlmProvider trait
│       │   ├── error.rs       # LlmError types
│       │   └── config.rs      # GenerationConfig
│       ├── auth/
│       │   ├── mod.rs
│       │   └── adc.rs         # ADC token management wrapper
│       ├── gemini/
│       │   ├── mod.rs
│       │   ├── client.rs      # GeminiClient implementation
│       │   ├── types.rs       # Gemini-specific request/response types
│       │   ├── mapper.rs      # Abstraction ↔ Gemini conversion
│       │   └── sse.rs         # SSE parser for Gemini format
│       ├── claude/
│       │   ├── mod.rs
│       │   ├── client.rs      # ClaudeClient implementation
│       │   ├── types.rs       # Claude-specific request/response types
│       │   ├── mapper.rs      # Abstraction ↔ Claude conversion
│       │   └── sse.rs         # SSE parser for Claude format
│       ├── tools/
│       │   ├── mod.rs
│       │   ├── executor.rs    # ToolExecutor trait
│       │   └── registry.rs    # FunctionRegistry implementation
│       └── http/
│           ├── mod.rs
│           ├── client.rs      # Shared HTTP client logic
│           └── retry.rs       # Retry/backoff logic
├── tests/
│   ├── message_db/            # Tests for message_db
│   │   └── ...
│   └── llm/                   # LLM tests
│       ├── integration/
│       │   ├── gemini_tests.rs
│       │   ├── claude_tests.rs
│       │   └── common.rs      # Shared test utilities
│       └── fixtures/
│           ├── gemini_responses.json
│           └── claude_responses.json
└── examples/
    ├── message_db/            # message_db examples
    │   └── ...
    └── llm/                   # LLM examples
        ├── simple_chat.rs
        ├── tool_use.rs
        └── parallel_tools.rs
```

## 11. Dependencies

```toml
[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP client
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Authentication
gcp_auth = "0.10"

# Streaming utilities
futures = "0.3"
futures-util = "0.3"
bytes = "1"

# Async trait support
async-trait = "0.1"

# Error handling
thiserror = "2"

# UUID generation for tool call IDs
uuid = { version = "1", features = ["v4", "serde"] }

[dev-dependencies]
# Testing
tokio-test = "0.4"
mockall = "0.13" # For mocking if needed

# Environment variables in tests
dotenvy = "0.15"
```

## 12. Implementation Phases

### Phase 1: Core Abstractions & Auth (Foundation)
1. Set up project structure:
   - Create `.gitignore` (include `.env`)
   - Create `.env.example` with template:
     ```
     GCP_PROJECT_ID=your-project-id
     GCP_LOCATION=us-central1
     ```
2. Define core types (`Message`, `ContentBlock`, `StreamEvent`, etc.)
   - Include `ToolDeclaration` in core types
   - Include tool-related content blocks (`ToolUse`, `ToolResult`)
3. Define `LlmProvider` trait
4. Implement ADC wrapper using `gcp_auth`
5. Write unit tests for type serialization/deserialization
6. **Deliverable**: Core abstractions compiled and tested

### Phase 2: Tool Execution Framework
**Rationale**: Build the tool system first so it's a first-class concern in provider implementations, not a bolt-on.

1. Define `ToolExecutor` trait
2. Implement `FunctionRegistry`
3. Implement function registration macro/helper
4. Write unit tests for tool execution
   - Test argument deserialization
   - Test result serialization
   - Test error handling in tool execution
5. **Deliverable**: Tool execution framework ready for integration

### Phase 3: Gemini Implementation (with Tool Support)
1. Implement Gemini request/response types
   - Include tool/function call structures from the start
2. Implement abstraction → Gemini mapper
   - Map `ToolDeclaration` to `functionDeclarations`
   - Map tool use/results in messages
3. Implement Gemini → abstraction mapper
   - Handle `functionCall` in response parts
4. Implement Gemini SSE parser
   - Parse function call responses
5. Implement `GeminiClient` struct and `LlmProvider` trait
6. Write unit tests for Gemini components
   - Include tool declaration conversion tests
   - Include tool call parsing tests
7. Write integration tests:
   - Simple text generation
   - Single tool call
   - Parallel tool calls
   - Tool use → tool result → continuation
8. **Deliverable**: Working Gemini client with full tool support

### Phase 4: Claude Implementation (with Tool Support)
1. Implement Claude request/response types
   - Include tool use structures from the start
2. Implement abstraction → Claude mapper
   - Map `ToolDeclaration` to Claude's `tools` array
   - Map tool use/results in messages
3. Implement Claude → abstraction mapper
   - Handle `tool_use` content blocks
   - Handle incremental tool argument streaming
4. Implement Claude SSE parser
   - Parse `content_block_start` with tool use
   - Accumulate `input_json_delta` fragments
   - Parse complete tool calls on `content_block_stop`
5. Implement `ClaudeClient` struct and `LlmProvider` trait
6. Write unit tests for Claude components
   - Include tool declaration conversion tests
   - Include incremental tool argument accumulation tests
7. Write integration tests:
   - Simple text generation
   - Single tool call
   - Parallel tool calls
   - Tool use → tool result → continuation
   - System prompt handling
8. **Deliverable**: Working Claude client with full tool support

### Phase 5: Error Handling & Retry Logic
1. Implement comprehensive error types
2. Implement retry logic with exponential backoff
3. Add error handling to all HTTP operations
4. Write tests for error scenarios
5. Write tests for retry behavior
6. **Deliverable**: Production-ready error handling

### Phase 6: Polish & Documentation
1. Add code documentation (rustdoc)
2. Create examples (simple chat, tool use, parallel tools)
   - Examples should show different approaches: hardcoded config, env vars, config structs
   - Demonstrate that `project_id` and `location` are just parameters
3. Write README with usage guide
   - Include setup instructions for running integration tests (copy `.env.example` to `.env`)
   - Clarify that `project_id` and `location` are library parameters, not hardcoded assumptions
4. Performance testing and optimization
5. Final integration test suite review
6. **Deliverable**: Production-ready library

## 13. Risk Mitigation

### Risk: API Changes
- **Mitigation**: Pin to specific API versions (`v1` for Vertex AI)
- **Mitigation**: Make `anthropic_version` configurable
- **Mitigation**: Comprehensive integration tests will detect breaking changes

### Risk: SSE Parsing Complexity
- **Mitigation**: Build robust stateful parser with extensive unit tests
- **Mitigation**: Test with malformed/truncated streams
- **Mitigation**: Use existing SSE parsing libraries if available (e.g., `eventsource-stream`)

### Risk: Tool Call JSON Accumulation (Claude)
- **Mitigation**: Careful state machine for partial JSON handling
- **Mitigation**: Test with various argument structures
- **Mitigation**: Clear error messages when JSON parsing fails

### Risk: Rate Limiting
- **Mitigation**: Exponential backoff with jitter
- **Mitigation**: Respect `Retry-After` header
- **Mitigation**: Allow users to implement custom rate limiting

### Risk: Authentication Failures
- **Mitigation**: Clear error messages for ADC setup issues
- **Mitigation**: Integration test documentation for credential setup
- **Mitigation**: Graceful fallback/error reporting

## 14. Future Enhancements (Out of Scope)

- Caching layer for responses
- Request/response logging to Cloud Logging
- Metrics/observability integration (OpenTelemetry)
- Support for additional models (GPT via Vertex, Gemma, etc.)
- Batch (non-streaming) mode
- Vision/multimodal input support
- Prompt caching (Claude feature)
- "Thinking mode" support (Gemini 2.5 feature)
- Global endpoint support (load balancing across regions)
- Custom retry policies
- Request deduplication
- WebSocket transport option

## 15. Success Criteria

The implementation will be considered successful when:

1. ✅ Both Gemini and Claude clients implement `LlmProvider` trait
2. ✅ Streaming text generation works for all supported models
3. ✅ Tool calling works (single and parallel) for both providers
4. ✅ Tool results can be sent back and conversation continues
5. ✅ Token usage metadata is accurately captured
6. ✅ ADC authentication works in all supported environments
7. ✅ Integration tests pass against live Vertex AI endpoints
8. ✅ Application code can swap providers without code changes (beyond client instantiation)
9. ✅ Error handling is robust and informative
10. ✅ Performance is acceptable (minimal overhead vs. raw HTTP)
