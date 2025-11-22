# Message DB Client Library Specification

Version: 1.0
Date: 2025-11-20

## 1. Introduction

### 1.1 Purpose

This specification defines the requirements for implementing a client library for Message DB, a PostgreSQL-based event store and message store designed for microservices, event sourcing, and pub/sub architectures.

### 1.2 Scope

This specification is language and framework agnostic. It describes the operations, data structures, and behaviors that a compliant Message DB client library must implement.

### 1.3 Message DB Overview

Message DB is a PostgreSQL-based message and event store that provides:
- Event sourcing capabilities
- Pub/Sub messaging patterns
- Stream and category-based message organization
- Optimistic concurrency control
- Consumer patterns for message processing
- Horizontal scalability through consumer groups

## 2. Core Concepts

### 2.1 Streams

A **stream** is a named sequence of messages with a specific identity. Stream names have a flexible structure that supports various naming patterns.

#### 2.1.1 Stream Name Structure

The general stream name pattern is:

```
{category}[:{type}[+{type}...]][-{id}]
```

**Components:**

1. **Category** (required): The primary classification of the stream
2. **Type** (optional): Specified with colon `:`, indicates specialized purpose (e.g., `:command`, `:snapshot`)
3. **Compound Types** (optional): Multiple types combined with plus `+`
4. **ID** (optional): Prefixed with hyphen `-`, identifies a specific entity instance

#### 2.1.2 Stream Name Examples

**Basic entity streams:**
- `account-123` - Account entity with ID "123"
- `account-456` - Account entity with ID "456"
- `withdrawal-abc-def` - Withdrawal entity with ID "abc-def" (IDs can contain hyphens)

**Category streams (no ID):**
- `account` - All account streams
- `withdrawal` - All withdrawal streams

**Streams with category types:**
- `account:command` - Command category stream
- `account:command-123` - Command stream for account 123
- `account:snapshot-789` - Snapshot stream for account 789
- `withdrawal:position-consumer-1` - Position tracking for withdrawal consumer

**Streams with compound types:**
- `account:command+position` - Command position tracking
- `transaction:event+audit` - Event stream with audit type
- `order:snapshot+v2` - Versioned snapshot stream

**Real-world patterns:**
- `category:v0-streamId` - Versioned stream (common pattern)
- `account:command:v1-123` - Versioned command stream
- `position-consumer1` - Simple position stream

#### 2.1.3 Stream Properties

- Stream names are case-sensitive strings
- Each message in a stream has a sequential position starting from 0
- Streams are append-only
- The hyphen `-` separates category (with types) from ID
- Colons `:` separate category from type(s)
- Plus signs `+` combine multiple types
- All components after the category are optional

### 2.2 Categories

A **category** is a logical grouping of related streams. The category is derived from the stream name by taking the portion before the first hyphen (`-`).

#### 2.2.1 Simple Categories

For simple stream names, the category is everything before the hyphen:

Examples:
- Stream `account-123` → category `account`
- Stream `account-456` → category `account`
- Stream `withdrawal-abc-def` → category `withdrawal`

#### 2.2.2 Category Types

Categories can include **types** that indicate specialized purposes. Types are appended to the category name using a colon (`:`) separator.

**Structure:** `{category}:{type}`

Examples:
- Stream `account:command-123` → category `account:command`
- Stream `account:snapshot-789` → category `account:snapshot`
- Stream `withdrawal:position-consumer-1` → category `withdrawal:position`
- Stream `order:command` → category `order:command` (no ID)

**Common category types:**
- `:command` - Command messages for the entity
- `:snapshot` - Snapshots of entity state
- `:position` - Consumer position tracking
- `:event` - Explicit event categorization
- `:v0`, `:v1`, `:v2` - Version indicators

#### 2.2.3 Compound Category Types

Multiple types can be combined using the plus (`+`) separator to create compound categories.

**Structure:** `{category}:{type}+{type}[+{type}...]`

Examples:
- Stream `account:command+position` → category `account:command+position`
- Stream `transaction:event+audit-123` → category `transaction:event+audit`
- Stream `order:snapshot+v2-456` → category `order:snapshot+v2`

#### 2.2.4 Category Extraction Rules

When extracting the category from a stream name:

1. Take everything before the first hyphen (`-`)
2. If no hyphen exists, the entire stream name is the category
3. Category includes all type qualifiers (colons and plus signs)

**Examples:**

| Stream Name | Category | ID |
|-------------|----------|-----|
| `account` | `account` | null |
| `account-123` | `account` | `123` |
| `account:command` | `account:command` | null |
| `account:command-123` | `account:command` | `123` |
| `account:command+position` | `account:command+position` | null |
| `account:v0-123` | `account:v0` | `123` |
| `transaction:event+audit-xyz` | `transaction:event+audit` | `xyz` |

#### 2.2.5 Category Usage

A category name (with or without types) can be used directly as a stream name for category-level messages:

- `account` - Category stream for all accounts
- `account:command` - Category stream for all account commands
- `account:position` - Category stream for account position tracking

When reading from a category using `get_category_messages`, all streams matching that category prefix are included:

- Reading category `account` includes: `account-123`, `account-456`, etc.
- Reading category `account:command` includes: `account:command-123`, `account:command-456`, etc.
- Categories with types do NOT match streams without those types:
  - Reading `account:command` does NOT include `account-123`
  - Reading `account` does NOT include `account:command-123`

### 2.3 Messages

Messages are the fundamental data units stored in Message DB. Each message represents either:
- An **event** (something that happened)
- A **command** (an instruction to do something)

### 2.4 Positions

Two types of positions track message location:

1. **Stream Position**: Sequential position within a specific stream (starts at 0)
2. **Global Position**: Sequential position across the entire message store

### 2.5 Consumer Groups

Consumer groups enable horizontal scaling by distributing messages from a category across multiple consumer instances. Messages are distributed based on a hash of the stream name.

## 3. Data Structures

### 3.1 Message Data (Write)

When writing a message to the store, the following fields are required or optional:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | UUID String | Yes | Unique identifier for the message |
| `type` | String | Yes | Message type/class name (e.g., "Withdrawn", "DepositRequested") |
| `data` | JSON Object | No | Business data payload |
| `metadata` | JSON Object | No | Infrastructural/mechanical data |

**Constraints:**
- `id` must be a valid UUID string
- `type` should follow naming conventions (typically PascalCase)
- `data` and `metadata` must be valid JSON objects (not arrays or primitives)

### 3.2 Message Data (Read)

When reading a message from the store, the following fields are populated:

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID String | Unique identifier for the message |
| `type` | String | Message type/class name |
| `data` | JSON Object | Business data payload |
| `metadata` | JSON Object | Infrastructural/mechanical data |
| `stream_name` | String | Name of the stream containing the message |
| `position` | Integer | Ordinal position in the stream (0-based) |
| `global_position` | Integer | Ordinal position in entire message store |
| `time` | Timestamp | UTC timestamp when message was written |

**Constraints:**
- `position` is 0-based and sequential within a stream
- `global_position` is monotonically increasing across the entire store
- `time` is set by the database at write time and cannot be specified by the client

### 3.3 Metadata Conventions

While metadata is freeform JSON, common conventional fields include:

| Field | Type | Purpose |
|-------|------|---------|
| `correlation_id` | UUID String | Links related messages across streams |
| `causation_id` | UUID String | ID of the message that caused this message |
| `reply_stream_name` | String | Stream name for replies (command pattern) |
| `schema_version` | String | Version of the message schema |

## 4. Core Operations

### 4.1 Write Message

Write a message to a stream with optional optimistic concurrency control.

**Operation Name:** `write_message`

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `id` | UUID String | Yes | - | Unique message identifier |
| `stream_name` | String | Yes | - | Target stream name |
| `type` | String | Yes | - | Message type |
| `data` | JSON Object | No | `{}` | Message data payload |
| `metadata` | JSON Object | No | `null` | Message metadata |
| `expected_version` | Integer | No | `null` | Expected current version for concurrency control |

**Returns:** Integer - the stream position of the written message

**Behavior:**

1. **Idempotency**: If a message with the same `id` already exists in the stream, the write is ignored and the existing position is returned
2. **Expected Version**:
   - If `expected_version` is `null`, no version check is performed
   - If `expected_version` is provided and doesn't match the current stream version, a concurrency error must be raised
   - The current stream version is the position of the last message (or -1 for empty streams)
3. **Atomic**: The write operation must be atomic
4. **Timestamp**: The database sets the `time` field automatically

**Errors:**

- Concurrency error if `expected_version` doesn't match
- Validation errors for invalid UUIDs or malformed JSON
- Database connection errors

**Example:**

```
write_message(
  id: "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d",
  stream_name: "account-123",
  type: "Withdrawn",
  data: { "amount": 50, "currency": "USD" },
  metadata: { "correlation_id": "xyz-789" },
  expected_version: 4
) -> 5
```

### 4.2 Get Stream Messages

Retrieve messages from a single stream.

**Operation Name:** `get_stream_messages`

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `stream_name` | String | Yes | - | Stream to read from |
| `position` | Integer | No | `0` | Starting position (inclusive) |
| `batch_size` | Integer | No | `1000` | Maximum messages to retrieve |
| `condition` | String | No | `null` | SQL WHERE condition for filtering |

**Returns:** List of Message Data (Read) structures, ordered by position

**Behavior:**

1. Returns messages starting from `position` (inclusive)
2. Returns up to `batch_size` messages
3. Messages are ordered by position ascending
4. If `condition` is provided, it's applied as a SQL WHERE clause
5. Empty list if no messages match criteria

**Example:**

```
get_stream_messages(
  stream_name: "account-123",
  position: 0,
  batch_size: 100
) -> [message1, message2, ...]
```

### 4.3 Get Category Messages

Retrieve messages from all streams in a category.

**Operation Name:** `get_category_messages`

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `category_name` | String | Yes | - | Category to read from |
| `position` | Integer | No | `1` | Starting global position (inclusive) |
| `batch_size` | Integer | No | `1000` | Maximum messages to retrieve |
| `correlation` | String | No | `null` | Correlation category for filtering |
| `consumer_group_member` | Integer | No | `null` | Consumer group member ID |
| `consumer_group_size` | Integer | No | `null` | Total consumer group size |
| `condition` | String | No | `null` | SQL WHERE condition for filtering |

**Returns:** List of Message Data (Read) structures, ordered by global position

**Behavior:**

1. Returns messages from all streams in the category
2. Starting from `position` (global position, inclusive)
3. Returns up to `batch_size` messages
4. Messages are ordered by global position ascending
5. **Correlation Filtering**: If `correlation` is provided, only returns messages where `metadata.correlation_id` matches a stream in the correlation category
6. **Consumer Groups**: If both `consumer_group_member` and `consumer_group_size` are provided:
   - Messages are distributed based on hash of stream name
   - Only messages assigned to this member are returned
   - `consumer_group_member` is 0-based (0 to size-1)

**Example:**

```
get_category_messages(
  category_name: "account",
  position: 1,
  batch_size: 100,
  consumer_group_member: 0,
  consumer_group_size: 3
) -> [message1, message2, ...]
```

### 4.4 Get Last Stream Message

Retrieve the most recent message from a stream.

**Operation Name:** `get_last_stream_message`

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `stream_name` | String | Yes | - | Stream to read from |
| `type` | String | No | `null` | Filter by message type |

**Returns:** Single Message Data (Read) structure, or null if no message exists

**Behavior:**

1. Returns the message with the highest position in the stream
2. If `type` is provided, returns the last message of that type
3. Returns null if stream is empty or no message matches the type

**Example:**

```
get_last_stream_message(
  stream_name: "account-123",
  type: "Withdrawn"
) -> message
```

### 4.5 Get Stream Version

Get the current version (position of last message) of a stream.

**Operation Name:** `stream_version`

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `stream_name` | String | Yes | - | Stream to check |

**Returns:** Integer - position of the last message, or null if stream doesn't exist

**Behavior:**

1. Returns the position of the last message in the stream
2. Returns null if the stream has no messages
3. For a stream with one message, returns 0
4. For a stream with n messages, returns n-1

**Example:**

```
stream_version("account-123") -> 4
```

## 5. Utility Operations

### 5.1 Extract ID from Stream Name

Extract the entity ID portion from a stream name.

**Operation Name:** `id`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to parse |

**Returns:** String - the ID portion after the first hyphen, or null if no hyphen exists

**Behavior:**

1. Splits stream name on first hyphen (`-`)
2. Returns everything after the first hyphen
3. Returns null if no hyphen exists
4. Works correctly with category types (colons and plus signs remain in category)

**Examples:**

```
id("account-123") -> "123"
id("account-123-456") -> "123-456"
id("account") -> null
id("account:command-123") -> "123"
id("account:v0-streamId") -> "streamId"
id("transaction:event+audit-xyz") -> "xyz"
id("account:command") -> null
```

### 5.2 Extract Cardinal ID from Stream Name

Extract the base entity ID (first segment after category) from a stream name. This is useful when IDs contain hyphens and you need just the primary identifier.

**Operation Name:** `cardinal_id`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to parse |

**Returns:** String - the cardinal ID, or null if no hyphen exists

**Behavior:**

1. Extracts the ID portion (everything after first hyphen)
2. Returns everything up to the second hyphen, or the full ID if no second hyphen
3. Returns null if no hyphen exists
4. Works correctly with category types

**Examples:**

```
cardinal_id("account-123") -> "123"
cardinal_id("account-123-456") -> "123"
cardinal_id("account") -> null
cardinal_id("account:command-123") -> "123"
cardinal_id("account:v0-streamId") -> "streamId"
cardinal_id("withdrawal:position-consumer-1") -> "consumer"
cardinal_id("account:command") -> null
```

### 5.3 Extract Category from Stream Name

Extract the category portion from a stream name, including any type qualifiers.

**Operation Name:** `category`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to parse |

**Returns:** String - the category portion (including types) before the first hyphen, or the entire string if no hyphen

**Behavior:**

1. Returns everything before the first hyphen (`-`)
2. If no hyphen exists, returns the entire stream name
3. Includes all category type qualifiers (colons and plus signs)

**Examples:**

```
category("account-123") -> "account"
category("account") -> "account"
category("account:command-123") -> "account:command"
category("account:v0-streamId") -> "account:v0"
category("transaction:event+audit-xyz") -> "transaction:event+audit"
category("account:command") -> "account:command"
category("withdrawal:position-consumer-1") -> "withdrawal:position"
```

### 5.4 Check if Stream Name is Category

Determine if a stream name represents a category (no ID portion).

**Operation Name:** `is_category`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to check |

**Returns:** Boolean - true if category, false if entity stream

**Behavior:**

1. Returns true if stream name contains no hyphen (`-`)
2. Returns false if stream name contains a hyphen
3. Category types (colons and plus signs) do not affect the result

**Examples:**

```
is_category("account") -> true
is_category("account-123") -> false
is_category("account:command") -> true
is_category("account:command-123") -> false
is_category("transaction:event+audit") -> true
is_category("transaction:event+audit-xyz") -> false
```

### 5.5 Hash Stream Name

Calculate a 64-bit hash for a stream name (used for consumer group distribution).

**Operation Name:** `hash_64`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to hash |

**Returns:** Integer - 64-bit hash value

**Behavior:**

1. Computes a consistent 64-bit hash of the stream name
2. Same stream name always produces same hash
3. Used internally for consumer group message distribution

**Example:**

```
hash_64("account-123") -> 8234567890123456
```

### 5.6 Extract Category Types from Stream Name (Optional)

Extract the type qualifiers from a category. This is an optional utility operation that some client libraries may choose to implement.

**Operation Name:** `get_category_types` or `types`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to parse |

**Returns:** List of Strings - the individual type qualifiers, or empty list if none

**Behavior:**

1. Extracts the category from the stream name
2. Identifies the type portion (everything after first colon)
3. Splits multiple types on the plus (`+`) separator
4. Returns list of individual types
5. Returns empty list if no types present

**Examples:**

```
get_category_types("account-123") -> []
get_category_types("account:command-123") -> ["command"]
get_category_types("account:v0-streamId") -> ["v0"]
get_category_types("transaction:event+audit-xyz") -> ["event", "audit"]
get_category_types("order:snapshot+v2+compressed") -> ["snapshot", "v2", "compressed"]
get_category_types("account") -> []
get_category_types("account:command") -> ["command"]
```

### 5.7 Extract Base Category from Stream Name (Optional)

Extract just the base category name without type qualifiers. This is an optional utility operation.

**Operation Name:** `get_base_category` or `base_category`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stream_name` | String | Yes | Stream name to parse |

**Returns:** String - the base category name without types

**Behavior:**

1. Extracts the category from the stream name
2. Returns everything before the first colon (`:`)
3. If no colon exists in category, returns the full category

**Examples:**

```
get_base_category("account-123") -> "account"
get_base_category("account:command-123") -> "account"
get_base_category("account:v0-streamId") -> "account"
get_base_category("transaction:event+audit-xyz") -> "transaction"
get_base_category("account") -> "account"
get_base_category("account:command") -> "account"
```

## 6. Consumer Pattern

### 6.1 Consumer Concept

A consumer continuously reads and processes messages from a category stream. The consumer pattern is typically implemented in client code using the core operations.

### 6.2 Consumer Requirements

A compliant consumer implementation should:

1. **Read Messages**: Use `get_category_messages` to retrieve messages
2. **Track Position**: Maintain the last processed global position
3. **Dispatch**: Route messages to registered handlers based on message type
4. **Poll**: Continuously poll for new messages
5. **Error Handling**: Handle message processing errors appropriately
6. **Position Storage**: Persist position for resumability

### 6.3 Position Tracking Pattern

Position tracking enables consumers to resume from where they left off:

1. Store position in a dedicated position stream
2. Position stream name convention: `{category}:position-{consumer_id}`
3. Write a position message after processing N messages (configurable)
4. Position message data: `{ "position": global_position }`
5. On startup, read last position from position stream

#### 6.3.1 Stream-Based Position Storage (Standard)

The standard approach stores consumer positions as messages in position streams:

**Advantages:**
- Uses Message DB's native event sourcing model
- Full history of position updates preserved
- No additional schema required
- Consistent with other message storage

**Disadvantages:**
- Requires reading stream to get latest position
- Position stream grows over time
- Slower read performance for position retrieval

**Implementation:**

```
# Write position
write_message(
  stream_name: "account:position-consumer-1",
  type: "PositionUpdated",
  data: { "position": 12345 }
)

# Read position on startup
last_message = get_last_stream_message("account:position-consumer-1")
position = last_message.data.position
```

#### 6.3.2 Table-Based Position Storage (Optimization)

For improved performance, client libraries may optionally support storing consumer positions in a separate PostgreSQL table optimized for fast reads and writes:

**Table Schema:**

```sql
CREATE TABLE consumer_positions (
  consumer_id VARCHAR PRIMARY KEY,
  position BIGINT NOT NULL,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_consumer_positions_updated ON consumer_positions(updated_at);
```

**Advantages:**
- Fast position reads (single row lookup via primary key)
- Fast position writes (UPDATE or INSERT with ON CONFLICT)
- No stream growth - only latest position stored
- Optimized for high-frequency position updates
- Lower storage overhead

**Disadvantages:**
- No position history
- Requires additional database schema
- Outside of Message DB's event sourcing model
- Need to manage table separately

**Implementation:**

```sql
-- Write/update position
INSERT INTO consumer_positions (consumer_id, position, updated_at)
VALUES ('account-consumer-1', 12345, CURRENT_TIMESTAMP)
ON CONFLICT (consumer_id)
DO UPDATE SET
  position = EXCLUDED.position,
  updated_at = EXCLUDED.updated_at;

-- Read position on startup
SELECT position
FROM consumer_positions
WHERE consumer_id = 'account-consumer-1';
```

**Hybrid Approach:**

Some implementations may combine both approaches:
- Use table-based storage for fast position reads during normal operation
- Periodically write position to stream for audit/history purposes
- On startup, read from table for speed
- Fall back to stream if table entry doesn't exist

**Consumer ID Conventions:**

When using table-based storage, consumer IDs should follow a consistent naming pattern:

```
{category}-{consumer_name}
{category}-{consumer_group}-{member_id}
```

Examples:
- `account-service-a`
- `account-worker-group-0`
- `withdrawal-processor-1`

**Note:** Table-based position storage is an optimization and not part of the core Message DB functionality. Client libraries should document whether they support this approach and provide guidance on when to use it.

### 6.4 Consumer Polling Pattern

Recommended polling behavior:

1. Read batch of messages from last position
2. Process each message
3. Update position after batch or N messages
4. If batch is empty, wait (polling interval)
5. Repeat

**Configuration Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `polling_interval_ms` | Integer | `100` | Wait time when no messages |
| `batch_size` | Integer | `10-100` | Messages per read |
| `position_update_interval` | Integer | `100` | Messages between position writes |

### 6.5 Consumer Groups Pattern

To implement consumer groups:

1. Assign each consumer instance a member ID (0 to size-1)
2. All instances use same `consumer_group_size`
3. Each instance specifies its `consumer_group_member` when calling `get_category_messages`
4. Message DB automatically distributes messages based on stream name hash
5. Each position stream is specific to the consumer instance

**Example:**

3 consumers processing "account" category:
- Consumer A: member 0, size 3, position stream "account:position-consumer-A"
- Consumer B: member 1, size 3, position stream "account:position-consumer-B"
- Consumer C: member 2, size 3, position stream "account:position-consumer-C"

### 6.6 Correlation-Based Pub/Sub

Implement pub/sub using correlation:

1. Command message includes `correlation_id` in metadata
2. Events resulting from command use same `correlation_id`
3. Consumer subscribing to replies uses `correlation` parameter
4. Only messages with matching correlation streams are returned

**Example:**

```
# Service A writes command
write_message(
  stream_name: "withdrawal-cmd-abc",
  metadata: { correlation_id: "xyz" }
)

# Service B processes and writes event
write_message(
  stream_name: "account-123",
  metadata: { correlation_id: "xyz" }
)

# Service A consumes correlated events
get_category_messages(
  category_name: "account",
  correlation: "withdrawal-cmd"  # Matches streams like withdrawal-cmd-*
)
```

## 7. Concurrency Control

### 7.1 Optimistic Concurrency

Message DB uses optimistic concurrency control through expected version:

1. Read stream to get current state
2. Make business decision
3. Write message with `expected_version` set to position of last read message
4. If another write occurred between read and write, write fails
5. Retry by re-reading and re-deciding

**Example:**

```
# Read current state
messages = get_stream_messages("account-123")
current_version = messages.last.position  # e.g., 4

# Make decision based on current state
...

# Write with expected version
try:
  write_message(
    stream_name: "account-123",
    expected_version: current_version,  # Must be 4
    ...
  )
catch ConcurrencyError:
  # Retry: re-read and re-decide
```

### 7.2 Idempotent Writes

Message DB provides automatic idempotency through message ID:

1. Generate deterministic message ID (e.g., from command ID + stream ID)
2. Attempting to write same ID to same stream is ignored
3. Returns position of existing message
4. Prevents duplicate processing

**Example:**

```
command_id = "abc-123"
stream_id = "456"
message_id = hash(command_id + stream_id)

# First write succeeds
write_message(id: message_id, ...) -> 5

# Retry with same ID is ignored
write_message(id: message_id, ...) -> 5
```

## 8. Database Transactions

### 8.1 Transaction Overview

Message DB operations are PostgreSQL database operations and can be executed within database transactions. Client libraries should support transaction management to enable:

1. **Atomic multi-message writes**: Write multiple messages atomically
2. **Transactional processing**: Combine reads, business logic, and writes in a single transaction
3. **Unit of work pattern**: Group related operations for consistency

### 8.2 Single Message Write (No Explicit Transaction)

For simple cases, write operations can be executed without explicit transaction management:

**Behavior:**
- Each `write_message` call executes in its own implicit transaction
- The operation is atomic - either the message is written or it fails completely
- Idempotency is maintained - duplicate message IDs are handled correctly
- Concurrency control via `expected_version` still applies

**Example:**

```
# Implicitly atomic - no explicit transaction needed
position = write_message(
  id: "abc-123",
  stream_name: "account-456",
  type: "Withdrawn",
  data: { amount: 50 }
)
```

**Use Cases:**
- Single event writes
- Independent command processing
- Position tracking updates

### 8.3 Multiple Message Writes in Single Transaction

Client libraries should support writing multiple messages within a single database transaction for atomicity across multiple writes.

**Requirements:**

1. **Transaction API**: Provide a way to begin, commit, and rollback transactions
2. **Connection Reuse**: All operations in a transaction must use the same database connection
3. **Atomicity**: Either all writes succeed or all fail
4. **Isolation**: Transaction isolation follows PostgreSQL's configured isolation level
5. **Error Handling**: On error, the entire transaction should be rolled back

**Example API Pattern:**

```
begin_transaction()
try:
  write_message(
    id: "msg-1",
    stream_name: "account-123",
    type: "Withdrawn",
    data: { amount: 50 }
  )

  write_message(
    id: "msg-2",
    stream_name: "account-456",
    type: "Deposited",
    data: { amount: 50 }
  )

  commit_transaction()
catch error:
  rollback_transaction()
  raise error
```

**Use Cases:**
- **Transfer operations**: Debit one account, credit another atomically
- **Saga coordination**: Write multiple correlated messages together
- **Aggregate consistency**: Write multiple events to same stream atomically
- **Process manager state**: Update process state and write commands atomically

### 8.4 Read-Process-Write Pattern in Transaction

The most common transactional pattern combines reading current state, processing business logic, and writing results:

**Pattern:**

```
begin_transaction()
try:
  # Read current state
  messages = get_stream_messages("account-123")
  current_version = stream_version("account-123")

  # Project current state
  account = project(messages)

  # Business logic
  if account.balance >= withdrawal_amount:
    # Write event with expected version
    write_message(
      id: "event-123",
      stream_name: "account-123",
      type: "Withdrawn",
      data: { amount: withdrawal_amount },
      expected_version: current_version
    )

    commit_transaction()
  else:
    rollback_transaction()
    raise InsufficientFundsError()
catch ConcurrencyError:
  rollback_transaction()
  # Retry entire transaction
catch error:
  rollback_transaction()
  raise error
```

**Important Considerations:**

1. **Optimistic Concurrency**: `expected_version` check happens at write time within the transaction
2. **Isolation Level**: Use appropriate isolation level (typically READ COMMITTED or REPEATABLE READ)
3. **Serialization Conflicts**: Higher isolation levels may cause serialization errors
4. **Retry Logic**: Application should retry on concurrency/serialization errors

### 8.5 Transaction Isolation Levels

Client libraries should support configuring PostgreSQL transaction isolation levels:

| Isolation Level | Behavior | Use Case |
|----------------|----------|----------|
| READ UNCOMMITTED | Not supported by PostgreSQL (falls back to READ COMMITTED) | N/A |
| READ COMMITTED | Default - sees committed data, no dirty reads | Most operations |
| REPEATABLE READ | Consistent snapshot, prevents non-repeatable reads | Complex projections |
| SERIALIZABLE | Fully serializable, may cause serialization failures | Critical consistency requirements |

**Recommendation**: Use READ COMMITTED for most operations. Message DB's optimistic concurrency control provides sufficient consistency without higher isolation levels.

### 8.6 Transaction Best Practices

#### 8.6.1 Keep Transactions Short

```
# Good - short transaction
begin_transaction()
  write_message(...)
  write_message(...)
commit_transaction()

# Bad - long transaction with external I/O
begin_transaction()
  write_message(...)
  call_external_api()  # DON'T DO THIS
  write_message(...)
commit_transaction()
```

**Guidelines:**
- Minimize transaction duration to reduce lock contention
- Don't perform I/O operations (HTTP calls, file I/O) within transactions
- Don't wait for user input within transactions
- Don't perform long computations within transactions

#### 8.6.2 Transaction Scope

```
# Good - transaction covers only database operations
account = project(messages)
result = perform_business_logic(account)  # Outside transaction

begin_transaction()
  write_message(result.event)
commit_transaction()

# Acceptable - business logic is fast
begin_transaction()
  messages = get_stream_messages("account-123")
  account = project(messages)  # Fast in-memory operation
  if account.balance >= amount:
    write_message(...)
commit_transaction()
```

#### 8.6.3 Error Handling in Transactions

```
begin_transaction()
try:
  write_message(...)
  write_message(...)
  commit_transaction()
catch ConcurrencyError:
  rollback_transaction()
  # Retry logic here
catch ValidationError:
  rollback_transaction()
  # Don't retry, propagate error
catch DatabaseError:
  rollback_transaction()
  # Check if transient, retry if appropriate
finally:
  ensure_transaction_closed()
```

### 8.7 Idempotency Across Transactions

Message idempotency (via message ID) works correctly across transaction boundaries:

**Scenario 1: Transaction commits, duplicate retry**

```
# First attempt
begin_transaction()
  write_message(id: "msg-1", ...) -> position 5
commit_transaction()

# Retry (e.g., network failure before client received response)
begin_transaction()
  write_message(id: "msg-1", ...) -> position 5 (idempotent)
commit_transaction()
```

**Scenario 2: Transaction rolls back, retry succeeds**

```
# First attempt
begin_transaction()
  write_message(id: "msg-1", ...)
  write_message(id: "msg-2", ...)  # Fails
rollback_transaction()  # msg-1 not written

# Retry
begin_transaction()
  write_message(id: "msg-1", ...) -> position 5 (new write)
  write_message(id: "msg-2", ...) -> position 6
commit_transaction()
```

### 8.8 Optimistic Concurrency in Transactions

Expected version checks interact with transactions:

**Behavior:**
- Version check occurs when `write_message` executes within transaction
- If version mismatch, the write raises an error immediately
- The transaction can catch and rollback
- No partial writes occur due to version mismatch

**Example:**

```
begin_transaction()
try:
  # These writes must all succeed for transaction to commit
  write_message(
    stream_name: "account-123",
    expected_version: 10,
    ...
  )

  write_message(
    stream_name: "account-456",
    expected_version: 5,
    ...
  )

  commit_transaction()
catch ConcurrencyError as e:
  rollback_transaction()
  # Determine which stream had conflict
  # Re-read and retry entire transaction
```

### 8.9 Position Tracking in Transactions

When writing consumer position updates:

**Pattern 1: Position update in same transaction as message processing**

```
begin_transaction()
  # Process messages, write resulting events
  write_message(stream_name: "account-123", ...)

  # Update position atomically with processing
  write_message(
    stream_name: "consumer:position-worker-1",
    data: { position: last_processed_position }
  )
commit_transaction()
```

**Benefits:**
- Position only advances if processing succeeds
- Guarantees at-least-once processing
- No message loss on failure

**Pattern 2: Position update in separate transaction**

```
# Process messages
begin_transaction()
  write_message(stream_name: "account-123", ...)
commit_transaction()

# Update position separately
begin_transaction()
  write_message(
    stream_name: "consumer:position-worker-1",
    data: { position: last_processed_position }
  )
commit_transaction()
```

**Considerations:**
- If position update fails but processing succeeded, message may be reprocessed
- Requires idempotent message handlers
- Allows position updates to be batched

### 8.10 Transaction API Requirements

Client libraries should provide a transaction API with these capabilities:

#### 8.10.1 Explicit Transaction Control

```
# Begin transaction
transaction = begin_transaction()

# Commit transaction
commit_transaction(transaction)

# Rollback transaction
rollback_transaction(transaction)

# Check transaction state
is_active(transaction) -> boolean
```

#### 8.10.2 Transaction Context/Closure Pattern

Many languages support a pattern that automatically handles commit/rollback:

```
with_transaction(lambda transaction:
  write_message(transaction, ...)
  write_message(transaction, ...)
  # Auto-commits on success, auto-rolls back on exception
)
```

**Requirements:**
- Automatically commit if closure succeeds
- Automatically rollback if closure raises exception
- Ensure transaction is closed even if error occurs
- Re-raise exception after rollback

#### 8.10.3 Connection Management

```
# Transactions must use a single connection
connection = get_connection()
transaction = begin_transaction(connection)
write_message(transaction, ...)
write_message(transaction, ...)
commit_transaction(transaction)
release_connection(connection)
```

**Requirements:**
- All operations in a transaction use the same connection
- Connection is held for transaction duration
- Connection is returned to pool after commit/rollback
- Handle connection failures gracefully

### 8.11 Read Operations in Transactions

Read operations can be performed within transactions for consistency:

**Use Case: Consistent multi-stream read**

```
begin_transaction()
  # Read multiple streams with consistent snapshot
  account_messages = get_stream_messages("account-123")
  ledger_messages = get_stream_messages("ledger-456")

  # Process with consistent view
  reconcile(account_messages, ledger_messages)
commit_transaction()
```

**Considerations:**
- Read operations don't acquire write locks
- Useful with REPEATABLE READ isolation for consistent snapshots
- Not typically necessary with Message DB's append-only model
- Consider read-only transaction optimization

### 8.12 Transaction Timeout

Client libraries should support transaction timeouts:

**Configuration:**

```
begin_transaction(timeout_ms: 5000)
```

**Behavior:**
- Transaction automatically rolled back if timeout exceeded
- Prevents transactions from holding connections indefinitely
- Different from statement timeout (applies to entire transaction)

**Recommended Default:** 30 seconds

### 8.13 Nested Transactions / Savepoints

PostgreSQL supports savepoints (partial rollback within transaction):

**Optional Support:**

```
begin_transaction()
  write_message(...)

  savepoint("sp1")
  try:
    write_message(...)  # May fail
  catch:
    rollback_to_savepoint("sp1")  # Partial rollback

  write_message(...)
commit_transaction()
```

**Use Cases:**
- Compensating actions
- Tentative operations
- Complex multi-step processing

**Note:** This is an advanced feature - not required for minimal implementation.

### 8.14 Transaction Example Scenarios

#### 8.14.1 Money Transfer (Dual Write)

```
begin_transaction()
  debit_event_id = generate_uuid()
  credit_event_id = generate_uuid()

  # Debit source account
  write_message(
    id: debit_event_id,
    stream_name: "account-123",
    type: "Withdrawn",
    data: { amount: 100, transfer_id: "txn-789" }
  )

  # Credit destination account
  write_message(
    id: credit_event_id,
    stream_name: "account-456",
    type: "Deposited",
    data: { amount: 100, transfer_id: "txn-789" }
  )

  # Record transfer
  write_message(
    id: generate_uuid(),
    stream_name: "transfer-789",
    type: "Completed",
    data: {
      from_account: "123",
      to_account: "456",
      amount: 100,
      debit_event_id: debit_event_id,
      credit_event_id: credit_event_id
    }
  )

commit_transaction()
```

#### 8.14.2 Aggregate with Multiple Events

```
begin_transaction()
  # Read current state
  messages = get_stream_messages("order-123")
  current_version = messages.last.position

  order = project(messages)

  # Business operation produces multiple events
  events = order.ship(shipping_address)

  # Write all events atomically
  for event in events:
    write_message(
      id: event.id,
      stream_name: "order-123",
      type: event.type,
      data: event.data,
      expected_version: current_version
    )
    current_version += 1

commit_transaction()
```

#### 8.14.3 Process Manager Coordination

```
begin_transaction()
  # Write completion event
  write_message(
    stream_name: "payment-process-abc",
    type: "PaymentReceived",
    ...
  )

  # Trigger next step via command
  write_message(
    stream_name: "shipping:command-xyz",
    type: "ShipOrder",
    metadata: { correlation_id: "abc" },
    ...
  )

commit_transaction()
```

### 8.15 Testing Transaction Behavior

Integration tests should verify:

1. **Atomic Multi-Write**: All messages written or none
2. **Rollback on Error**: Failed transaction doesn't write any messages
3. **Idempotency in Transactions**: Duplicate message IDs handled correctly
4. **Concurrency Errors**: Version mismatch rolls back entire transaction
5. **Isolation**: Concurrent transactions don't interfere incorrectly
6. **Connection Handling**: Connections properly released after commit/rollback

### 8.16 Transaction Documentation Requirements

Client library documentation should include:

1. **Transaction API reference**: All transaction methods
2. **Examples**: Common transaction patterns
3. **Best practices**: When and how to use transactions
4. **Isolation levels**: How to configure and when to use
5. **Error handling**: Transaction-specific error scenarios
6. **Performance implications**: Impact of transactions on throughput

### 8.17 Minimal Transaction Support

For minimal implementation, client libraries must support:

1. **Single operation (implicit transaction)**: Each write is atomic
2. **Explicit transaction begin/commit/rollback**: Basic transaction control
3. **Multiple writes in transaction**: Atomic multi-message writes
4. **Connection reuse**: Same connection for all operations in transaction
5. **Error handling**: Proper rollback on error

Optional but recommended:
- Transaction context/closure pattern
- Configurable isolation levels
- Transaction timeout
- Savepoints

## 9. Database Schema

### 9.1 Messages Table

Client libraries must understand the underlying schema:

**Table:** `message_store.messages`

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | UUID | PRIMARY KEY | Message identifier |
| `stream_name` | TEXT | NOT NULL | Stream name |
| `type` | TEXT | NOT NULL | Message type |
| `position` | BIGINT | NOT NULL | Position in stream |
| `global_position` | BIGSERIAL | PRIMARY KEY | Global position |
| `data` | JSONB | | Message data |
| `metadata` | JSONB | | Message metadata |
| `time` | TIMESTAMP | NOT NULL | Write timestamp (UTC) |

**Indexes:**

1. `messages_id`: Unique index on `id`
2. `messages_stream`: Unique index on `(stream_name, position)`
3. `messages_category`: Index supporting category queries

### 9.2 Server Functions

Message DB operations are implemented as PostgreSQL functions in the `message_store` schema:

- `message_store.write_message(...)`
- `message_store.get_stream_messages(...)`
- `message_store.get_category_messages(...)`
- `message_store.get_last_stream_message(...)`
- `message_store.stream_version(...)`
- `message_store.id(...)`
- `message_store.cardinal_id(...)`
- `message_store.category(...)`
- `message_store.is_category(...)`
- `message_store.hash_64(...)`

Client libraries must call these functions, not access tables directly.

## 10. Connection and Configuration

### 10.1 Database Connection

Client libraries should support:

1. **PostgreSQL connection parameters**:
   - Host
   - Port (default: 5432)
   - Database name (typically "message_store")
   - Username
   - Password
   - SSL mode

2. **Connection pooling**: Recommended for production use

3. **Connection string**: Support standard PostgreSQL connection string format

### 10.2 Configuration Options

Recommended configuration options:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `database_url` | String | - | PostgreSQL connection string |
| `schema_name` | String | `message_store` | Schema containing functions |
| `default_batch_size` | Integer | `1000` | Default batch size for reads |
| `command_timeout_ms` | Integer | `30000` | Timeout for database operations |
| `session_timezone` | String | `UTC` | Database session timezone |

## 11. Error Handling

### 11.1 Error Types

Client libraries should distinguish these error categories:

1. **Concurrency Errors**
   - Expected version mismatch
   - Should trigger retry logic

2. **Validation Errors**
   - Invalid UUID format
   - Invalid JSON
   - Missing required fields

3. **Connection Errors**
   - Database unreachable
   - Authentication failure
   - Timeout

4. **Not Found Errors**
   - Stream doesn't exist (for version check)
   - Message doesn't exist

5. **Database Errors**
   - SQL errors
   - Constraint violations

### 11.2 Error Handling Best Practices

1. Retry transient errors (connection issues)
2. Don't retry validation errors
3. Application code decides whether to retry concurrency errors
4. Provide clear error messages with context
5. Log errors with sufficient detail for debugging

## 12. Type Conversions

### 12.1 UUIDs

- Message DB uses UUID strings in standard format
- Format: `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- Client libraries should validate UUID format
- Case-insensitive but should preserve original case

### 12.2 JSON Serialization

- `data` and `metadata` are JSONB in PostgreSQL
- Client libraries must serialize/deserialize correctly
- Preserve field order when possible
- Handle null vs missing fields appropriately

### 12.3 Timestamps

- Timestamps are UTC
- Client libraries should return timestamps in appropriate native type
- Preserve microsecond precision if supported

### 12.4 Integers

- Positions are 64-bit integers
- Client libraries must support full range
- Positions start at 0 for stream position
- Global positions start at 1

## 13. Testing Requirements

### 13.1 Unit Tests

Client libraries should include tests for:

1. Parsing utilities (id, category, cardinal_id, is_category)
2. Data structure validation
3. Error handling
4. Type conversions

### 13.2 Integration Tests

Client libraries should test against actual Message DB:

1. Writing messages to streams
2. Reading from streams
3. Reading from categories
4. Expected version enforcement
5. Idempotent writes
6. Consumer group distribution
7. Correlation filtering
8. Position tracking

### 13.3 Test Database

#### 13.3.1 Docker-Based Testing (Recommended)

**Docker Image:**

The recommended approach for local development and integration testing is to use the official Message DB Docker image:

```
ethangarofolo/message-db:1.3.1
```

This image provides a fully configured PostgreSQL instance with Message DB schema and functions pre-installed.

**Docker Compose Configuration:**

A proven `docker-compose.yml` configuration for testing:

```yaml
services:
  messagedb:
    image: ethangarofolo/message-db:1.3.1
    ports:
      - "5433:5432"  # Use 5433 externally to avoid conflicts with local postgres
    environment:
      POSTGRES_PASSWORD: message_store_password
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 10
```

**Connection Details:**

When using the above configuration:

| Parameter | Value |
|-----------|-------|
| Host | `localhost` |
| Port | `5433` |
| Database | `message_store` |
| Username | `postgres` |
| Password | `message_store_password` |
| Connection String | `postgresql://postgres:message_store_password@localhost:5433/message_store` |

**Integration Testing Requirements:**

Client libraries **must** include comprehensive integration tests that run against a real Message DB instance. This is absolutely vital for ensuring correctness and compatibility.

**Testing Library Support:**

Most programming languages have testing libraries that support starting and stopping Docker containers during test execution:

| Language | Library/Tool | Description |
|----------|--------------|-------------|
| Rust | `testcontainers` | Start/stop containers in tests |
| Java | `Testcontainers` | Official Testcontainers library |
| Python | `testcontainers-python` | Python port of Testcontainers |
| Go | `testcontainers-go` | Go implementation |
| JavaScript/TypeScript | `testcontainers-node` | Node.js implementation |
| .NET | `Testcontainers` | .NET library |
| Ruby | `docker-api` gem | Docker API wrapper |

**Test Setup Pattern:**

```
1. Start Message DB container before tests
2. Wait for healthcheck to pass
3. Run integration tests against container
4. Stop and remove container after tests
5. Each test should clean up its data or use unique stream names
```

**Benefits of Docker-Based Testing:**

- Consistent test environment across developers and CI/CD
- No manual database installation required
- Isolation from local development databases
- Tests against actual Message DB implementation
- Easy version management (pin to specific image tag)
- Fast container startup (typically 2-5 seconds)

#### 13.3.2 Alternative: Manual Database Setup

If Docker is not available, provide scripts to:

- Install PostgreSQL
- Install Message DB schema
- Configure test database
- Reset database between test runs

#### 13.3.3 Test Isolation Best Practices

- Use unique stream name prefixes per test (e.g., `test{uuid}-account-123`)
- Clean up test data after each test
- Support running tests in parallel
- Don't assume empty database state
- Tests should be idempotent and repeatable

## 14. Documentation Requirements

Client library documentation should include:

1. **Installation instructions**
2. **Quick start guide**
3. **API reference** for all operations
4. **Examples** for common patterns:
   - Writing events
   - Reading streams
   - Implementing consumers
   - Consumer groups
   - Correlation-based pub/sub
   - Position tracking
   - Optimistic concurrency
5. **Error handling guide**
6. **Performance tuning** recommendations
7. **Migration guide** from other versions/libraries

## 15. Performance Considerations

### 15.1 Batch Sizes

- Larger batches reduce database round trips
- Smaller batches reduce memory usage
- Recommended: 10-1000 depending on message size

### 15.2 Connection Pooling

- Reuse database connections
- Configure pool size based on concurrency needs
- Monitor connection pool saturation

### 15.3 Indexing

- Message DB indexes support efficient queries
- Don't add custom indexes without understanding implications
- Category queries use specialized indexes

### 15.4 Position Updates

- Don't update position after every message
- Update every N messages (e.g., 100)
- Balance between resumability and write overhead

## 16. Security Considerations

### 16.1 SQL Injection

- All parameters to server functions are properly escaped by PostgreSQL
- Don't construct dynamic SQL - use parameterized function calls
- `condition` parameter accepts SQL but is used in prepared statements

### 16.2 Authentication

- Use PostgreSQL authentication mechanisms
- Support SSL/TLS connections
- Don't log connection strings with passwords

### 16.3 Authorization

- Message DB relies on PostgreSQL role-based security
- Client libraries should support role-based connections
- Consider separate read-only vs read-write connections

## 17. Extensibility

### 17.1 Custom Metadata

- Client libraries should allow arbitrary metadata fields
- Provide conveniences for conventional fields (correlation_id, causation_id)
- Don't restrict metadata schema

### 17.2 Message Serialization

- Support pluggable serialization for message data
- Default to JSON
- Allow custom serializers for specific types

### 17.3 Middleware/Interceptors

Consider providing hooks for:
- Pre-write message transformation
- Post-read message transformation
- Logging/tracing
- Metrics collection

## 18. Version Compatibility

### 18.1 Message DB Versions

- Client libraries should document compatible Message DB versions
- Check database schema version on connection
- Fail fast if incompatible version detected

### 18.2 Function: get_message_store_version

**Operation Name:** `message_store_version`

**Parameters:** None

**Returns:** String - version number of Message DB schema

**Example:**

```
message_store_version() -> "1.2.3"
```

### 18.3 Backward Compatibility

- Maintain backward compatibility within major versions
- Deprecate features before removing
- Provide migration guides for breaking changes

## 19. Example Workflows

### 19.1 Command Handler Workflow

```
1. Receive command
2. Generate message ID from command ID
3. Read entity stream to get current state
4. Project events onto entity
5. Make business decision
6. Write event with expected version
7. Handle concurrency errors by retrying from step 3
```

### 19.2 Consumer Workflow

```
1. Read last position from position stream
2. Read batch of category messages from last position
3. For each message:
   a. Dispatch to handler based on type
   b. Handle message
   c. Update in-memory position
4. Write position to position stream
5. If batch was full, goto 2 (more messages available)
6. If batch was empty, wait (polling interval), goto 2
```

### 19.3 Pub/Sub Workflow

```
Service A (Publisher):
1. Write command to command stream with correlation_id
2. Write event to entity stream with same correlation_id

Service B (Subscriber):
1. Read category messages with correlation filter
2. Receive only events correlated with command stream
3. Process events
```

## 20. Minimal Implementation

A minimal viable client library must implement:

1. **Core operations**:
   - `write_message`
   - `get_stream_messages`
   - `get_category_messages`

2. **Data structures**:
   - Message data (read and write)

3. **Utilities**:
   - `category` (for parsing)
   - `id` (for parsing)

4. **Connection**:
   - PostgreSQL connection support

5. **Error handling**:
   - Concurrency errors
   - Validation errors
   - Connection errors

Optional but recommended:
- `get_last_stream_message`
- `stream_version`
- Consumer helpers
- Position tracking helpers
- Consumer group support

## 21. Reference Implementation

The canonical reference implementations are:

1. **Ruby**: message-db-ruby gem (part of Eventide project)
2. **PostgreSQL**: The database functions themselves

Client library authors should consult these implementations for clarification on behavior and edge cases.

## 22. Compliance Checklist

A compliant Message DB client library should:

- [ ] Implement all core write and read operations
- [ ] Support optimistic concurrency control via expected version
- [ ] Support idempotent writes via message ID
- [ ] Support database transactions (begin, commit, rollback)
- [ ] Support multiple writes in single transaction
- [ ] Parse stream names correctly (category, id, cardinal_id)
- [ ] Handle all documented error types
- [ ] Support consumer groups
- [ ] Support correlation-based filtering
- [ ] Provide data structures for messages (read and write)
- [ ] Support JSON serialization for data and metadata
- [ ] Handle UUIDs correctly
- [ ] Work with UTC timestamps
- [ ] Support 64-bit integer positions
- [ ] Provide connection pooling or guidance
- [ ] Include integration tests (including transaction tests)
- [ ] Document all public APIs
- [ ] Provide examples of common patterns (including transactional patterns)
- [ ] Check Message DB version compatibility

## Appendix A: Stream Naming Conventions

### A.1 Entity Streams

Basic entity streams without category types.

**Format:** `{category}-{id}`

**Examples:**
- `account-123` - Account entity
- `user-abc-def-ghi` - User with compound ID
- `order-550e8400-e29b-41d4-a716-446655440000` - Order with UUID ID

### A.2 Command Streams

Streams for command messages using the `:command` category type.

**Format:** `{category}:command-{id}`

**Examples:**
- `account:command-789` - Commands for account 789
- `withdrawal:command-xyz` - Withdrawal commands
- `order:command-550e8400` - Order commands

**Alternative (without type):** `{category}-{id}` where category name itself implies commands
- `withdrawal-xyz` - If "withdrawal" represents the command category

### A.3 Position Streams

Streams for tracking consumer position, using the `:position` category type.

**Format:** `{source_category}:position-{consumer_id}`

**Examples:**
- `account:position-service-a` - Service A's position in account category
- `withdrawal:position-worker-1` - Worker 1's position in withdrawal category
- `order:position-consumer-group-0` - Consumer group member 0's position

### A.4 Snapshot Streams

Streams for entity snapshots, using the `:snapshot` category type.

**Format:** `{category}:snapshot-{id}`

**Examples:**
- `account:snapshot-123` - Snapshots for account 123
- `order:snapshot-456` - Snapshots for order 456
- `aggregate:snapshot:v2-789` - Versioned snapshot

### A.5 Versioned Streams

Streams using version indicators in the category type.

**Format:** `{category}:v{version}-{id}`

**Examples:**
- `account:v0-123` - Version 0 of account stream
- `order:v1-456` - Version 1 of order stream
- `transaction:v2-789` - Version 2 of transaction stream

### A.6 Compound Type Streams

Streams combining multiple category types with plus (`+`) separator.

**Format:** `{category}:{type}+{type}[+{type}...]-{id}`

**Examples:**
- `account:command+position` - Command position tracking (no ID)
- `transaction:event+audit-xyz` - Audited event stream
- `order:snapshot+v2+compressed-123` - Versioned, compressed snapshot
- `account:command+replay-456` - Replay of commands

### A.7 Category Streams

Category-level streams without entity IDs.

**Format:** `{category}[:{type}[+{type}...]]`

**Examples:**
- `account` - All account streams
- `account:command` - All account command streams
- `account:position` - Position tracking category
- `transaction:event+audit` - Audited transaction events

**Special categories:**
- `$all` - Special category representing all messages (if supported by implementation)

## Appendix B: Metadata Conventions

### B.1 Correlation

```json
{
  "correlation_id": "abc-123"
}
```

Links related messages across streams for pub/sub.

### B.2 Causation

```json
{
  "causation_id": "message-id-that-caused-this"
}
```

Tracks message that directly caused this message.

### B.3 Reply Stream

```json
{
  "reply_stream_name": "withdrawal:reply-abc"
}
```

Indicates where to send replies to a command.

### B.4 Schema Version

```json
{
  "schema_version": "2"
}
```

Version of the message data schema.

### B.5 Trace ID

```json
{
  "trace_id": "distributed-trace-id"
}
```

For distributed tracing integration.

## Appendix C: SQL Condition Examples

The `condition` parameter accepts SQL WHERE clause syntax:

### C.1 Filter by Type

```sql
type = 'Withdrawn'
```

### C.2 Filter by Multiple Types

```sql
type IN ('Deposited', 'Withdrawn')
```

### C.3 Filter by Metadata

```sql
metadata->>'correlation_id' = 'abc-123'
```

### C.4 Filter by Time

```sql
time > '2025-01-01 00:00:00'::timestamp
```

### C.5 Complex Conditions

```sql
type = 'Withdrawn' AND (data->>'amount')::numeric > 100
```

## Appendix D: PostgreSQL Function Signatures

For reference, the PostgreSQL function signatures:

```sql
message_store.write_message(
  _id varchar,
  _stream_name varchar,
  _type varchar,
  _data jsonb,
  _metadata jsonb DEFAULT NULL,
  _expected_version bigint DEFAULT NULL
) RETURNS bigint

message_store.get_stream_messages(
  _stream_name varchar,
  _position bigint DEFAULT 0,
  _batch_size bigint DEFAULT 1000,
  _condition varchar DEFAULT NULL
) RETURNS SETOF message_store.message

message_store.get_category_messages(
  _category_name varchar,
  _position bigint DEFAULT 1,
  _batch_size bigint DEFAULT 1000,
  _correlation varchar DEFAULT NULL,
  _consumer_group_member bigint DEFAULT NULL,
  _consumer_group_size bigint DEFAULT NULL,
  _condition varchar DEFAULT NULL
) RETURNS SETOF message_store.message

message_store.get_last_stream_message(
  _stream_name varchar,
  _type varchar DEFAULT NULL
) RETURNS message_store.message

message_store.stream_version(
  _stream_name varchar
) RETURNS bigint

message_store.id(
  _stream_name varchar
) RETURNS varchar

message_store.cardinal_id(
  _stream_name varchar
) RETURNS varchar

message_store.category(
  _stream_name varchar
) RETURNS varchar

message_store.is_category(
  _stream_name varchar
) RETURNS boolean

message_store.hash_64(
  _stream_name varchar
) RETURNS bigint

message_store.message_store_version()
RETURNS varchar
```

---

**End of Specification**
