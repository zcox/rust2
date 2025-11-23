# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust monorepo containing three main components:

1. **AI Agent Chat API** - SSE-based chat server with streaming responses (`src/main.rs`, `src/handlers/`, `src/sse.rs`)
2. **Message DB Client Library** - High-performance async client for PostgreSQL-based event store (`src/message_db/`)
3. **LLM Abstraction Layer** - Unified interface for Claude and Gemini on Vertex AI (`src/llm/`)

## Build and Test Commands

```bash
# Development
cargo check                    # Fast syntax/type checking
cargo clippy                   # Linting
cargo fmt                      # Format code
cargo build                    # Debug build
cargo build --release          # Optimized release build

# Running
cargo run                      # Start HTTP server on localhost:3030

# Testing
cargo test                     # All tests
cargo test message_db          # Only Message DB tests
cargo test --test integration_test    # Integration tests (requires Docker)

# Message DB Examples
docker-compose up -d           # Start Message DB (port 5433)
cargo run --example phase1_demo
cargo run --example writing_events
cargo run --example reading_streams
cargo run --example transactions
cargo run --example optimistic_concurrency
cargo run --example consumer_example
cargo run --example consumer_groups

# Documentation
cargo doc --open               # Generate and view API docs
```

## Architecture

### HTTP Server (src/)
- **Entry point**: `main.rs` - Warp server on port 3030
- **Routes**: `routes.rs` - Defines `/api/v1/threads/{threadId}` endpoints
- **Handlers**: `handlers/get_thread.rs` and `handlers/send_message.rs`
- **SSE Streaming**: `sse.rs` - Server-Sent Events for real-time streaming
- **Models**: `models.rs` - Thread, Message, and SSE event types

The API uses SSE to stream agent responses in chunks with event types: `agent_text`, `tool_call`, `tool_response`, and `done`.

### Message DB Client (src/message_db/)
Event sourcing library implementing Message DB patterns:

- **Connection**: `connection.rs` - Connection pool with deadpool-postgres
- **Client**: `client.rs` - Main `MessageDbClient` API
- **Operations**: `operations/{write,read,query}.rs` - Core database operations
- **Transactions**: `transaction.rs` - Atomic multi-message writes with optimistic concurrency
- **Consumer**: `consumer/` - Consumer pattern with position tracking and consumer groups
- **Utils**: `utils/parsing.rs` - Stream name parsing (`category()`, `id()`, `cardinal_id()`, `is_category()`)
- **Types**: `types/message.rs` - `Message` and `WriteMessage` structs

**Key Concepts**:
- Stream names: `category-id` or `category:type-id` (e.g., `account:command-123`)
- Categories: Stream name without ID (e.g., `account:command`)
- Position tracking: Global position for resumable message processing
- Consumer groups: Multiple consumers with coordinated position tracking

**Connection Details** (docker-compose.yml):
- Database: `message_store`
- Port: 5433 (external)
- User: `postgres`
- Password: `message_store_password`

### LLM Abstraction Layer (src/llm/)
Unified interface for Claude and Gemini on Google Cloud Vertex AI:

- **Core**: `core/{provider,types,config,error}.rs` - `LlmProvider` trait and shared types
- **Gemini**: `gemini/{client,mapper,sse,types}.rs` - Gemini implementation
- **Claude**: `claude/{client,mapper,sse,types}.rs` - Claude implementation
- **Auth**: `auth/adc.rs` - Application Default Credentials for GCP
- **Tools**: `tools/{registry,executor}.rs` - Function calling support

**Architecture Pattern**:
- Both Claude and Gemini implement the `LlmProvider` trait with a single `stream_generate()` method
- Provider-specific mappers convert between unified types and provider-specific formats
- SSE parsers handle streaming responses from each provider
- Authentication uses GCP Application Default Credentials (ADC)

**Models Available**:
- Claude: Sonnet 4.5 (`claude-sonnet-4-5@20250929`), Haiku 4.5 (`claude-haiku-4-5@20251001`)
- Gemini: 2.5 Pro, 2.5 Flash, 2.5 Flash Lite

**Unified Types** (`core/types.rs`):
- `GenerateRequest` - Request with messages, tools, and config
- `Message` - Chat message with role and content
- `StreamEvent` - Streaming response events (MessageStart, ContentDelta, ToolUse, MessageStop)
- `ContentBlock` - Text or tool use content
- `ToolDeclaration` - Function calling schema

## Code Organization

- **Library exports**: `src/lib.rs` exposes all three components as public modules
- **Binary entry point**: `src/main.rs` runs the HTTP server
- **Tests**: `tests/` for integration tests, inline unit tests in source files
- **Examples**: `examples/` demonstrates Message DB usage patterns

## Development Notes

### Testing with Docker
Integration tests use testcontainers to spin up Message DB automatically. The `docker-compose.yml` provides a persistent instance for manual testing and examples.

### Message DB Stream Names
Always use the parsing utilities (`category()`, `id()`, etc.) from `message_db::utils` rather than string manipulation. They handle all edge cases including type qualifiers (`:command`, `:event`), compound types (`+audit`), and position streams (`:position`).

### LLM Provider Extension
To add a new LLM provider:
1. Create module in `src/llm/{provider}/`
2. Implement `LlmProvider` trait
3. Create mappers for provider-specific types → unified types
4. Implement SSE parser if provider uses streaming

### Error Handling
- Message DB: Custom `Error` enum with typed variants (see ERROR_HANDLING_GUIDE.md)
- LLM: `LlmError` enum for auth, network, parsing, and API errors
- Both use `Result<T, Error>` type aliases

### SSE Event Format
The chat API uses a specific SSE format with `event:` and `data:` fields. See README.md API documentation for exact event types and JSON schemas.
