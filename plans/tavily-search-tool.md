# Tavily Search Tool Implementation Plan

## Overview

Implement a Tavily Search tool as a built-in agent tool that enables web search capabilities. The tool will follow the same pattern as the existing calculator tool, using the `#[tool]` macro for automatic registration and schema generation.

## Tavily API Details

### Endpoint
- **Base URL**: `https://api.tavily.com`
- **Endpoint**: `POST /search`
- **Authentication**: Bearer token with API key

### Request Format
```json
{
  "query": "search query string",
  "topic": "general",           // optional: general, news, finance
  "search_depth": "basic",      // optional: basic, advanced
  "max_results": 5,             // optional: 0-20, default 5
  "include_answer": false,      // optional: generate AI answer
  "include_raw_content": false, // optional: include full webpage content
  "include_images": false       // optional: perform image search
}
```

### Response Format
```json
{
  "query": "string",
  "answer": "string (if include_answer=true)",
  "results": [
    {
      "title": "string",
      "url": "string",
      "content": "string (snippet)",
      "score": 0.95
    }
  ],
  "response_time": 1.23,
  "credits_used": 1
}
```

## Implementation Steps

### 1. Create Tool File Structure

**File**: `src/llm/tools/builtin/tavily_search.rs`

Following the calculator pattern, the file should contain:
- Args struct with JsonSchema
- Result struct with Serialize
- Main tool function with #[tool] macro
- Unit tests

### 2. Define Type Structures

#### SearchArgs
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TavilySearchArgs {
    /// The search query to execute
    pub query: String,

    /// Maximum number of search results to return (0-20, default: 5)
    #[serde(default)]
    pub max_results: Option<u8>,

    /// Search topic category (default: general)
    #[serde(default)]
    pub topic: Option<SearchTopic>,

    /// Search depth (default: basic)
    #[serde(default)]
    pub search_depth: Option<SearchDepth>,

    /// Whether to include an AI-generated answer (default: false)
    #[serde(default)]
    pub include_answer: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchTopic {
    General,
    News,
    Finance,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchDepth {
    Basic,
    Advanced,
}
```

#### SearchResult
```rust
#[derive(Debug, Serialize)]
pub struct TavilySearchResult {
    /// The original query
    pub query: String,

    /// AI-generated answer (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,

    /// Search results
    pub results: Vec<SearchResultItem>,

    /// API response time
    pub response_time: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub content: String,
    pub score: f64,
}
```

### 3. Implement Tool Function

```rust
#[tool(
    description = "Search the web for current information using Tavily's search API. Returns relevant web pages with titles, URLs, and content snippets.",
    crate_path = "crate"
)]
pub async fn tavily_search(args: TavilySearchArgs) -> Result<TavilySearchResult, String> {
    // Implementation details below
}
```

**Key Implementation Details:**
- Function must be `async` since it makes HTTP requests
- Use `reqwest` crate for HTTP client
- Read `TAVILY_API_KEY` from environment variable
- Return errors as `String` for tool macro compatibility

### 4. HTTP Client Implementation

**Dependencies to add to `Cargo.toml`:**
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
```

**Implementation approach:**
1. Read API key from env var at runtime: `std::env::var("TAVILY_API_KEY")`
2. Return error if API key is missing
3. Create async reqwest client
4. Build request body from TavilySearchArgs with defaults:
   - max_results: unwrap_or(5)
   - topic: unwrap_or("general")
   - search_depth: unwrap_or("basic")
   - include_answer: unwrap_or(false)
   - include_raw_content: false (hardcoded, not exposed to agent)
   - include_images: false (hardcoded, not exposed to agent)
5. Send POST request to `https://api.tavily.com/search` with Bearer auth header
6. Handle HTTP status codes (200 = success, 4xx/5xx = errors)
7. Parse JSON response into TavilySearchResult
8. Return result or descriptive error message

### 5. Error Handling

Return descriptive error strings for:
- Missing API key: "TAVILY_API_KEY environment variable not set"
- HTTP request failures: "Failed to connect to Tavily API: {error}"
- Invalid responses: "Invalid response from Tavily API: {error}"
- API errors: "Tavily API error: {status_code} - {message}"
- Rate limiting: "Tavily API rate limit exceeded"

### 6. Update Module Exports

**File**: `src/llm/tools/builtin/mod.rs`

```rust
pub mod calculator;
pub mod tavily_search;

pub use calculator::{calculate, CalculatorArgs, CalculatorResult};
pub use tavily_search::{tavily_search, TavilySearchArgs, TavilySearchResult};
```

### 7. Register Tool in main.rs

Add to imports:
```rust
use rust2::llm::tools::builtin::tavily_search::tavily_search_tool;
```

Add to tool registration:
```rust
registry
    .register(tavily_search_tool::registration())
    .expect("Failed to register tavily search tool");
```

### 8. Environment Variable Setup

**Status**: API key already configured in `.env` file.

The `.env` file contains:
```
TAVILY_API_KEY=<actual-key-already-set>
```

**Documentation needed**:
- Add `.env.example` entry showing `TAVILY_API_KEY=tvly-YOUR_API_KEY_HERE`
- Document in README.md that Tavily API key is required for web search functionality

## Design Decisions

Based on requirements analysis, the following design decisions have been made:

### 1. HTTP Client: Async (reqwest)
**Decision**: Use async reqwest client for non-blocking HTTP requests.

**Rationale**: Better server performance and resource utilization. The tool function will be async.

**Verification**: ✅ Phase 0 confirmed that async functions are fully supported by the tool system (see Technical Considerations section below).

### 2. Result Caching: Not Initially
**Decision**: No caching in first implementation.

**Rationale**: Start simple and ensure fresh results. Caching can be added later if API costs become a concern.

**Future**: Consider adding in-memory cache with TTL in future enhancement.

### 3. Default max_results: 5
**Decision**: Use Tavily's default of 5 results.

**Rationale**: Balanced between providing sufficient information and minimizing token usage and API costs. The agent can request more or fewer results if needed.

### 4. API Key Management: Simple
**Decision**: Check for `TAVILY_API_KEY` environment variable presence, return error if missing during execution.

**Rationale**: Simple and sufficient for initial implementation. No need for complex validation or rotation support initially.

**Implementation**:
- Check for env var at runtime when tool is called
- Return clear error message if missing
- No startup validation or test requests

## Technical Considerations

### Async Tool Execution ✅

**Status**: VERIFIED - Async functions are fully supported!

**Phase 0 Investigation Results:**

1. **ToolExecutor Trait** (`src/llm/tools/executor.rs:24`):
   - Uses `#[async_trait]`
   - Execute method is async: `async fn execute(...) -> Result<String, String>`
   - ✅ Natively supports async execution

2. **FunctionRegistry** (`src/llm/tools/registry.rs`):
   - Line 115: `register_async_tool()` - dedicated method for async functions
   - Line 233: `register_sync_tool()` - dedicated method for sync functions
   - Line 189: `register()` - convenience method that works with both
   - Both async and sync functions wrapped as `BoxFuture<'static, Result<String, String>>`
   - ✅ Fully supports both async and sync tools

3. **#[tool] Macro** (`rust2_tool_macros/src/lib.rs`):
   - Line 125: Auto-detects async with `input_fn.sig.asyncness.is_some()`
   - Lines 128-157: Generates async wrapper for async functions
   - Lines 158-188: Generates sync wrapper for sync functions
   - ✅ Automatically handles both patterns

**Async Wrapper Pattern (generated by macro)**:
```rust
let wrapper = move |args_json: serde_json::Value| {
    let args = serde_json::from_value::<Args>(args_json)?;
    let future = execute(args);  // Call async function
    Box::pin(async move {
        match future.await {      // Await the result
            Ok(result) => serde_json::to_string(&result),
            Err(e) => Err(e),
        }
    }) as BoxFuture<'static, _>
};
```

**Test Evidence**:
- Registry tests include async examples (lines 438-459)
- Demonstrates async/await with tokio::time::sleep working correctly

**Conclusion**: No modifications to the tool system needed. We can implement the Tavily search tool as a standard async function.

### API Key Security

- Never commit actual API keys to version control
- API key loaded at runtime from `TAVILY_API_KEY` environment variable
- Return clear error message if key is missing during tool execution
- Document in README and .env.example

### Rate Limiting & Costs

- Tavily API has rate limits and credit system
- Default max_results of 5 provides cost control
- Document rate limits in README
- Agent can adjust max_results parameter to control costs per query

### Response Size

- Search results can be large if include_raw_content is true
- Default to excluding raw content to keep responses manageable
- Content snippets are sufficient for most agent use cases
- Let LLM decide when it needs full content via include_raw_content parameter

## Testing Strategy

### Unit Tests

Create tests in `tavily_search.rs`:

1. **Test request serialization**: Verify TavilySearchArgs serializes correctly
2. **Test response deserialization**: Verify parsing of Tavily API responses
3. **Test error cases**: Verify error messages for common failures

### Integration Tests

**Approach**: Use real Tavily API with API key from `.env` file.

**Unit tests** (in `tavily_search.rs`):
- Use mocked HTTP responses (`mockito` or similar)
- Test serialization/deserialization
- Test error handling without network calls

**Integration tests** (in `tests/` directory):
- Use actual Tavily API with real API key from `.env`
- Test end-to-end functionality with real searches
- Verify actual response parsing and handling
- May want to use `#[ignore]` attribute to avoid running on every test run (to save API credits)

**Example program** (optional):
- Create `examples/tavily_search_demo.rs` for manual testing and demonstration

### Example Program

Create `examples/tavily_search_demo.rs`:
```rust
// Demonstrates using the Tavily search tool
// Usage: cargo run --example tavily_search_demo
```

## Implementation Phases

### Phase 0: Verify Async Support ✅ COMPLETE
- ✅ Checked `ToolExecutor` trait - uses `#[async_trait]`, fully async
- ✅ Checked `FunctionRegistry` - has `register_async_tool()` method
- ✅ Checked `#[tool]` macro - auto-detects and handles async functions
- ✅ Verified with existing test cases
- **Result**: Full async support confirmed, no tool system modifications needed

### Phase 1: Basic Implementation with Tests
- Implement type structures (TavilySearchArgs, TavilySearchResult, enums)
- Implement async tool function with query parameter
- All optional parameters supported with defaults (5 results, general topic, basic depth)
- Error handling (API key check, network errors, API errors)
- **Write unit tests**: serialization/deserialization, error cases with mocks
- **Write integration test**: basic search with real API (can use `#[ignore]`)

### Phase 2: Full Feature Support with Tests
- Verify all optional parameters work correctly (topic, depth, include_answer, etc.)
- Better error messages for API-specific errors
- Add logging/tracing for debugging
- **Expand unit tests**: test different parameter combinations with mocks
- **Expand integration tests**: test all parameters with real API
- Create example program for manual demonstration (optional)

### Phase 3: Integration & Documentation
- Register in main.rs
- Update CLAUDE.md with tool documentation
- Add TAVILY_API_KEY to .env.example
- Update README.md with web search capability documentation
- Test with actual agent queries end-to-end

## Expected File Changes

1. **New file**: `src/llm/tools/builtin/tavily_search.rs` (~200 lines)
2. **Modified**: `src/llm/tools/builtin/mod.rs` (add exports)
3. **Modified**: `src/main.rs` (add tool registration)
4. **Modified**: `Cargo.toml` (add reqwest dependency if not present)
5. **Modified**: `.env` (add TAVILY_API_KEY)
6. **Modified**: `README.md` (document tool and env var)
7. **Modified**: `CLAUDE.md` (add tool to documentation)
8. **New file**: `examples/tavily_search_demo.rs` (optional)

## Success Criteria

- [x] **Phase 0**: Verified tool system supports async functions
- [ ] Tool compiles without errors
- [ ] Tool registers successfully in FunctionRegistry
- [ ] Tool appears in LLM tool declarations with correct schema
- [ ] Agent can invoke tool with natural language queries
- [ ] Search results are returned in expected format
- [ ] All parameters (max_results, topic, search_depth, include_answer) work correctly
- [ ] Error cases are handled gracefully (missing API key, network errors, API errors)
- [ ] Documentation is complete (code comments, README, CLAUDE.md)
- [ ] Example or tests demonstrate functionality
- [ ] TAVILY_API_KEY is documented in .env.example

## Future Enhancements

1. **Caching**: Cache search results to reduce API calls and costs
2. **Parallel searches**: Support multiple queries in one call
3. **Result filtering**: Allow filtering by score threshold
4. **Domain-specific search**: Add support for site-specific searches
5. **Image search**: Implement image search support
6. **News monitoring**: Create a separate tool for news-specific searches
7. **Content extraction**: Integrate with Tavily's /extract endpoint
8. **Research mode**: Use Tavily's /research endpoint for comprehensive analysis
