# Rust Message DB Client Library - Technical Plan

**Date:** 2025-11-20
**Version:** 1.0

## Executive Summary

This document outlines the technical decisions and implementation plan for a Rust client library for Message DB, a PostgreSQL-based event store and message store designed for microservices, event sourcing, and pub/sub architectures.

## Database Library Selection

### Decision: tokio-postgres + deadpool-postgres

After comprehensive research and evaluation, we have selected **tokio-postgres** with **deadpool-postgres** for connection pooling as the foundation for the Rust Message DB client library.

### Evaluation Criteria

The selection was based on Message DB's specific requirements:
- PostgreSQL function calls (not direct table access)
- Transaction support (begin/commit/rollback)
- Connection pooling for production use
- Async operations for consumer polling patterns
- JSONB support for message data and metadata
- Proper parameter binding and type handling

### Libraries Evaluated

#### 1. tokio-postgres ✅ SELECTED
- **Performance:** Up to 50% faster than SQLx in benchmarks
- **Query Pipelining:** 20% performance boost potential (supported)
- **Transactions:** Full support including savepoints
- **JSONB:** Excellent serde_json integration
- **Async:** Native Tokio runtime integration
- **Fit:** Perfect for calling PostgreSQL functions directly
- **Pooling:** Via deadpool-postgres (recommended) or bb8

#### 2. SQLx
- **Performance:** Slower than tokio-postgres (50% in large reads)
- **Built-in Pooling:** Yes, convenient
- **Compile-time Checks:** Verifies SQL at compile time
- **Query Pipelining:** Not supported
- **Fit:** Good, but unnecessary abstraction layer
- **Trade-off:** Convenience vs performance

#### 3. Diesel
- **Type Safety:** Strong via Rust type system
- **ORM Features:** Not needed for Message DB's function-based API
- **Async:** Only via third-party diesel-async
- **Fit:** Poor - DSL doesn't match Message DB's raw SQL functions
- **Performance:** ORM overhead not beneficial
- **Decision:** Not suitable for this use case

### Key Reasons for tokio-postgres

1. **Performance is Critical**
   - Event sourcing requires high throughput
   - Consumer polling patterns benefit from lower overhead
   - Query pipelining support for additional optimization
   - Benchmarks show significant performance advantage

2. **Natural Architecture Fit**
   - Message DB uses PostgreSQL functions, not ORM patterns
   - Direct function calls: `message_store.write_message()`, `message_store.get_stream_messages()`, etc.
   - No DSL abstraction mismatch
   - Raw SQL approach is ideal

3. **First-Class Transaction Support**
   - Clean transaction API matching PostgreSQL semantics
   - Savepoint support for nested transactions
   - Critical for atomic multi-message writes
   - Maps directly to Message DB transaction requirements

4. **Production-Ready Ecosystem**
   - deadpool-postgres provides excellent connection pooling
   - Statement caching built-in
   - Fast recycling method (no test queries)
   - Mature, battle-tested in production

5. **Simplicity and Control**
   - Smaller dependency tree
   - Predictable behavior
   - Easier performance debugging
   - Direct access to PostgreSQL features

### Trade-offs

**No Compile-Time Query Checking**
- Acceptable: Message DB has well-defined API
- Mitigation: Comprehensive integration tests against real database
- Tests use Message DB Docker image for consistency

**External Pooling Dependency**
- Minimal cost: deadpool-postgres is lightweight
- Standard practice in Rust ecosystem
- Better performance than built-in alternatives

**More Boilerplate**
- Solution: Abstract in client library API
- Users get clean, ergonomic interface
- Internal complexity hidden

## Implementation Plan

### Core Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-postgres = { version = "0.7", features = ["with-serde_json-1", "with-uuid-1", "with-chrono-0_4"] }
deadpool-postgres = "0.14"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
testcontainers = "0.15"
```

### Architecture Overview

```
┌─────────────────────────────────────┐
│   Message DB Client Library API    │
│  (Clean, ergonomic Rust interface) │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│    Core Operations Layer            │
│  - write_message                    │
│  - get_stream_messages              │
│  - get_category_messages            │
│  - get_last_stream_message          │
│  - stream_version                   │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│   Transaction Management Layer      │
│  - begin_transaction                │
│  - commit_transaction               │
│  - rollback_transaction             │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│   Connection Pool Layer             │
│     (deadpool-postgres)             │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│      tokio-postgres Driver          │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│    PostgreSQL Message DB            │
│   (message_store.* functions)       │
└─────────────────────────────────────┘
```

### Module Structure

```
message-db-client/
├── src/
│   ├── lib.rs                 # Public API
│   ├── client.rs              # Main client struct
│   ├── connection.rs          # Connection pool management
│   ├── transaction.rs         # Transaction support
│   ├── operations/
│   │   ├── mod.rs
│   │   ├── write.rs           # write_message
│   │   ├── read.rs            # get_stream_messages, get_category_messages
│   │   └── query.rs           # get_last_stream_message, stream_version
│   ├── types/
│   │   ├── mod.rs
│   │   ├── message.rs         # Message structs
│   │   └── stream.rs          # Stream name utilities
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── parsing.rs         # id, category, cardinal_id, is_category
│   │   └── hash.rs            # hash_64
│   ├── consumer/
│   │   ├── mod.rs
│   │   ├── consumer.rs        # Consumer implementation
│   │   ├── position.rs        # Position tracking
│   │   └── group.rs           # Consumer groups
│   └── error.rs               # Error types
├── tests/
│   └── integration/           # Integration tests with Docker
└── examples/                  # Usage examples
```

### Core Types

**Message Data Structures:**
```rust
// Write message
pub struct WriteMessage {
    pub id: Uuid,
    pub stream_name: String,
    pub message_type: String,
    pub data: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub expected_version: Option<i64>,
}

// Read message
pub struct Message {
    pub id: Uuid,
    pub stream_name: String,
    pub message_type: String,
    pub data: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub position: i64,
    pub global_position: i64,
    pub time: chrono::DateTime<chrono::Utc>,
}
```

**Client Interface:**
```rust
pub struct MessageDbClient {
    pool: deadpool_postgres::Pool,
}

impl MessageDbClient {
    pub async fn new(config: Config) -> Result<Self>;
    pub async fn write_message(&self, msg: WriteMessage) -> Result<i64>;
    pub async fn get_stream_messages(&self, opts: StreamOptions) -> Result<Vec<Message>>;
    pub async fn get_category_messages(&self, opts: CategoryOptions) -> Result<Vec<Message>>;
    pub async fn transaction(&self) -> Result<Transaction>;
}
```

**Transaction Support:**
```rust
pub struct Transaction<'a> {
    transaction: deadpool_postgres::Transaction<'a>,
}

impl<'a> Transaction<'a> {
    pub async fn write_message(&mut self, msg: WriteMessage) -> Result<i64>;
    pub async fn commit(self) -> Result<()>;
    pub async fn rollback(self) -> Result<()>;
}
```

### Implementation Phases

#### Phase 1: Foundation (Week 1) ✅ COMPLETE
**Implementation:**
- [x] Project setup and dependencies
- [x] Connection pool configuration
- [x] Basic error types
- [x] Stream name parsing utilities (category, id, cardinal_id, is_category)

**Testing:**
- [x] Set up Docker testcontainers infrastructure
- [x] Unit tests for stream name parsing utilities
  - Test category extraction with various stream formats
  - Test id extraction with compound IDs
  - Test cardinal_id extraction
  - Test is_category with edge cases
- [x] Integration test for connection pool setup
  - Test successful connection to Message DB
  - Test connection pool exhaustion/recovery
  - Test connection timeout handling
- [x] Unit tests for error type conversions

**Completion Date:** 2025-11-20
**Test Results:** All 31 tests passing (6 parsing tests, 5 config tests, 5 integration tests, 9 doc tests, 6 client tests)
**Documentation:** See [MESSAGE_DB_PHASE1_SUMMARY.md](MESSAGE_DB_PHASE1_SUMMARY.md) for detailed completion report.

#### Phase 2: Core Operations (Week 2)
**Implementation:**
- [ ] write_message implementation
- [ ] get_stream_messages implementation
- [ ] get_category_messages implementation
- [ ] get_last_stream_message implementation
- [ ] stream_version implementation

**Testing:**
- [ ] Integration tests for write_message
  - Test basic message write
  - Test idempotent writes (duplicate message ID)
  - Test expected_version enforcement (optimistic concurrency)
  - Test JSONB data and metadata serialization
  - Test UUID handling
- [ ] Integration tests for get_stream_messages
  - Test reading from empty stream
  - Test reading with position offset
  - Test batch size limits
  - Test SQL condition filtering
- [ ] Integration tests for get_category_messages
  - Test category message retrieval
  - Test consumer group distribution
  - Test correlation filtering
- [ ] Integration tests for get_last_stream_message
  - Test with empty stream
  - Test with type filtering
- [ ] Integration tests for stream_version
  - Test with non-existent stream
  - Test after multiple writes

#### Phase 3: Transactions (Week 3)
**Implementation:**
- [ ] Transaction begin/commit/rollback
- [ ] Transactional write operations
- [ ] Savepoint support (optional)
- [ ] Transaction timeout configuration

**Testing:**
- [ ] Integration tests for transaction atomicity
  - Test multiple writes commit together
  - Test rollback prevents all writes
  - Test partial failure rolls back entire transaction
- [ ] Integration tests for transactional concurrency
  - Test expected_version in transaction
  - Test concurrent transactions with conflicts
  - Test serialization errors
- [ ] Integration tests for savepoints (if implemented)
  - Test partial rollback to savepoint
  - Test nested savepoints
- [ ] Integration tests for transaction timeout
  - Test timeout triggers rollback
  - Test connection cleanup after timeout
- [ ] Integration tests for idempotency in transactions
  - Test duplicate message ID in committed transaction
  - Test duplicate message ID in rolled-back transaction

#### Phase 4: Consumer Support (Week 4) ✅ COMPLETE
**Implementation:**
- [x] Consumer polling pattern
- [x] Position tracking (stream-based)
- [x] Consumer group support
- [x] Correlation-based filtering

**Testing:**
- [x] Integration tests for consumer polling
  - Test continuous polling with new messages
  - Test polling with empty category
  - Test batch processing
- [x] Integration tests for position tracking
  - Test position read/write to position stream
  - Test resume from last position
  - Test position update intervals
- [x] Integration tests for consumer groups
  - Test message distribution across group members
  - Test each member receives unique messages
  - Test consistent hash distribution
- [x] Integration tests for correlation filtering
  - Test correlation-based message filtering (1 test ignored pending clarification)
  - Test pub/sub pattern with correlation
- [x] End-to-end consumer scenario tests
  - Test command handler workflow
  - Test event consumer workflow

**Completion Date:** 2025-11-20
**Test Results:** 10 passing integration tests (1 ignored), total 107 passing tests across all phases
**Documentation:** See [MESSAGE_DB_PHASE4_SUMMARY.md](MESSAGE_DB_PHASE4_SUMMARY.md) for detailed completion report.

#### Phase 5: Documentation & Examples (Week 5)
- [ ] API documentation (rustdoc)
- [ ] Quick start guide
- [ ] Usage examples
  - Writing events example
  - Reading streams example
  - Consumer implementation example
  - Transaction pattern examples
  - Consumer group example
  - Position tracking example
  - Optimistic concurrency example
- [ ] Error handling guide
- [ ] Performance tuning recommendations
- [ ] Migration guide (if applicable)

#### Phase 6: Optimization & Polish (Week 6)
**Implementation:**
- [ ] Query pipelining exploration
- [ ] Table-based position storage (optional)
- [ ] Connection pool tuning

**Testing:**
- [ ] Performance benchmarks
  - Benchmark write throughput
  - Benchmark read throughput
  - Benchmark consumer polling latency
  - Compare with tokio-postgres baseline
- [ ] Stress tests
  - High-volume writes
  - Multiple concurrent consumers
  - Connection pool under load
- [ ] Edge case testing
  - Very large messages
  - Very large batch sizes
  - Long-running transactions

### Testing Strategy

**Integration Tests:**
- Use `testcontainers` crate to start Message DB Docker container
- Docker image: `ethangarofolo/message-db:1.3.1`
- Test against real PostgreSQL with Message DB functions
- Test all core operations
- Test transaction behavior (atomicity, rollback, concurrency)
- Test consumer patterns
- Test error scenarios

**Unit Tests:**
- Stream name parsing (category, id, cardinal_id, is_category)
- Data structure validation
- Type conversions
- Hash functions

**Docker Compose Configuration:**
```yaml
services:
  messagedb:
    image: ethangarofolo/message-db:1.3.1
    ports:
      - "5433:5432"
    environment:
      POSTGRES_PASSWORD: message_store_password
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 10
```

### Performance Considerations

1. **Connection Pooling**
   - Configure max connections based on workload
   - Use deadpool's fast recycling method
   - Monitor pool saturation

2. **Batch Sizes**
   - Default: 1000 messages per read
   - Configurable based on message size
   - Balance memory vs round trips

3. **Position Updates**
   - Update every N messages (default: 100)
   - Balance resumability vs write overhead
   - Consider table-based storage for high-frequency updates

4. **Query Pipelining**
   - Investigate tokio-postgres pipelining features
   - Potential 20% performance improvement
   - Document usage patterns

### Documentation Requirements

- Installation instructions
- Quick start guide
- API reference for all operations
- Examples:
  - Writing events
  - Reading streams
  - Implementing consumers
  - Consumer groups
  - Transaction patterns
  - Position tracking
  - Optimistic concurrency
- Error handling guide
- Performance tuning recommendations
- Migration guide from other libraries

### Success Criteria

**Compliance Checklist:**
- ✅ All core operations implemented per spec
- ✅ Optimistic concurrency control via expected_version
- ✅ Idempotent writes via message ID
- ✅ Database transaction support
- ✅ Multiple writes in single transaction
- ✅ Stream name parsing utilities
- ✅ All documented error types handled
- ✅ Consumer group support
- ✅ Correlation-based filtering
- ✅ Proper JSONB handling
- ✅ UUID support
- ✅ UTC timestamp handling
- ✅ 64-bit integer position support
- ✅ Connection pooling
- ✅ Comprehensive integration tests
- ✅ Complete API documentation
- ✅ Usage examples

## Conclusion

The combination of tokio-postgres and deadpool-postgres provides the optimal foundation for a high-performance, production-ready Message DB client library in Rust. This approach prioritizes performance, direct control over PostgreSQL features, and a clean mapping to Message DB's function-based architecture while maintaining simplicity and reliability.
