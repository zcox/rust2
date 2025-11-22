# Error Handling Guide

This guide explains how to handle errors when using the Message DB Rust client library.

## Error Types

The library defines a comprehensive `Error` enum with the following variants:

### 1. ConcurrencyError

Occurs when an optimistic concurrency check fails during `write_message`.

```rust
Error::ConcurrencyError {
    stream_name: String,
    expected_version: i64,
    actual_version: Option<i64>,
}
```

**When it occurs:**
- Writing with `expected_version` that doesn't match the current stream version
- Another process modified the stream between your read and write

**How to handle:**
```rust
match client.write_message(event).await {
    Ok(position) => {
        println!("Write succeeded at position {}", position);
    }
    Err(Error::ConcurrencyError { stream_name, expected_version, actual_version }) => {
        eprintln!("Concurrency conflict on stream {}", stream_name);
        eprintln!("Expected version: {}, Actual: {:?}", expected_version, actual_version);
        // Retry: re-read stream, recalculate, and write again
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

**Best practices:**
- Implement retry logic (with maximum attempts)
- Re-read stream state before retrying
- Recalculate business logic with fresh data
- Consider exponential backoff for retries

### 2. ValidationError

Occurs when input data fails validation.

```rust
Error::ValidationError(String)
```

**When it occurs:**
- Invalid UUID format
- Malformed JSON in data or metadata
- Missing required fields
- Invalid stream name format

**How to handle:**
```rust
match client.write_message(event).await {
    Ok(position) => { /* ... */ }
    Err(Error::ValidationError(msg)) => {
        eprintln!("Validation failed: {}", msg);
        // Fix the input and retry, or return error to caller
        // Do NOT retry without fixing the input
    }
    Err(e) => { /* ... */ }
}
```

**Best practices:**
- Validate input before calling Message DB operations
- Return validation errors to the user/caller
- Do not retry validation errors without fixing the input
- Log validation errors for debugging

### 3. DatabaseError

Occurs when database operations fail.

```rust
Error::DatabaseError(String)
```

**When it occurs:**
- SQL execution failure
- Constraint violations
- Transaction errors
- Query timeouts

**How to handle:**
```rust
match client.write_message(event).await {
    Ok(position) => { /* ... */ }
    Err(Error::DatabaseError(msg)) => {
        eprintln!("Database error: {}", msg);

        // Check if it's a transient error
        if msg.contains("connection") || msg.contains("timeout") {
            // Retry transient errors
        } else {
            // Log and propagate non-transient errors
        }
    }
    Err(e) => { /* ... */ }
}
```

**Best practices:**
- Distinguish between transient and permanent errors
- Retry transient errors (connection issues, timeouts)
- Log all database errors with context
- Monitor error rates for operational issues

### 4. ConnectionError

Occurs when database connection fails.

```rust
Error::ConnectionError(String)
```

**When it occurs:**
- Cannot connect to PostgreSQL
- Connection pool exhausted
- Network issues
- Authentication failures

**How to handle:**
```rust
match MessageDbClient::new(config).await {
    Ok(client) => { /* ... */ }
    Err(Error::ConnectionError(msg)) => {
        eprintln!("Connection failed: {}", msg);
        // Check configuration, network, and database availability
        // Implement retry with exponential backoff
    }
    Err(e) => { /* ... */ }
}
```

**Best practices:**
- Validate connection string format
- Check database availability before starting application
- Implement connection retry logic with backoff
- Monitor connection pool health
- Configure appropriate pool size

### 5. NotFoundError

Occurs when a requested resource doesn't exist.

```rust
Error::NotFoundError(String)
```

**When it occurs:**
- Stream doesn't exist when checking version
- Message doesn't exist
- Reading from non-existent category

**How to handle:**
```rust
match client.stream_version(stream_name).await {
    Ok(Some(version)) => {
        println!("Stream version: {}", version);
    }
    Ok(None) => {
        println!("Stream does not exist");
        // This is not an error - handle as empty stream
    }
    Err(Error::NotFoundError(msg)) => {
        eprintln!("Not found: {}", msg);
    }
    Err(e) => { /* ... */ }
}
```

**Best practices:**
- Distinguish between `None` return (normal) and errors
- Handle missing streams/messages gracefully
- Consider whether missing data is an error in your context

## Common Error Handling Patterns

### Pattern 1: Retry with Exponential Backoff

```rust
use tokio::time::{sleep, Duration};

async fn write_with_retry<F>(
    mut operation: F,
    max_retries: u32,
) -> Result<i64, Error>
where
    F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i64, Error>>>>,
{
    let mut attempt = 0;
    let base_delay = Duration::from_millis(100);

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(Error::ConcurrencyError { .. }) if attempt < max_retries => {
                let delay = base_delay * 2u32.pow(attempt - 1);
                eprintln!("Retry attempt {}/{} after {:?}", attempt, max_retries, delay);
                sleep(delay).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Pattern 2: Read-Process-Write with Retry

```rust
async fn process_with_optimistic_concurrency(
    client: &MessageDbClient,
    stream_name: &str,
    max_retries: u32,
) -> Result<i64, Error> {
    for attempt in 1..=max_retries {
        // 1. Read current state
        let options = StreamReadOptions::new(stream_name);
        let messages = client.get_stream_messages(options).await?;

        // 2. Calculate new state
        let current_state = project_state(&messages);

        // 3. Make business decision
        let new_event = make_decision(current_state)?;

        // 4. Get current version
        let version = client.stream_version(stream_name).await?;

        // 5. Write with expected version
        let event = new_event.with_expected_version(version.unwrap_or(-1));

        match client.write_message(event).await {
            Ok(position) => return Ok(position),
            Err(Error::ConcurrencyError { .. }) if attempt < max_retries => {
                eprintln!("Concurrency conflict, retrying... ({}/{})", attempt, max_retries);
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Err(Error::DatabaseError("Max retries exceeded".to_string()))
}
```

### Pattern 3: Transaction Error Handling

```rust
async fn atomic_operation(
    client: &MessageDbClient,
) -> Result<(), Error> {
    let mut txn = client.begin_transaction().await?;

    match perform_operations(&mut txn).await {
        Ok(()) => {
            txn.commit().await?;
            Ok(())
        }
        Err(e) => {
            eprintln!("Operation failed: {}, rolling back", e);
            txn.rollback().await?;
            Err(e)
        }
    }
}

async fn perform_operations(txn: &mut Transaction) -> Result<(), Error> {
    // Perform multiple writes
    txn.write_message(event1).await?;
    txn.write_message(event2).await?;
    Ok(())
}
```

### Pattern 4: Graceful Degradation

```rust
async fn get_data_with_fallback(
    client: &MessageDbClient,
    stream_name: &str,
) -> Vec<Message> {
    let options = StreamReadOptions::new(stream_name);

    match client.get_stream_messages(options).await {
        Ok(messages) => messages,
        Err(Error::ConnectionError(msg)) => {
            eprintln!("Connection error: {}, returning cached data", msg);
            get_cached_data(stream_name)
        }
        Err(e) => {
            eprintln!("Error reading stream: {}, returning empty", e);
            Vec::new()
        }
    }
}
```

### Pattern 5: Error Context with anyhow

```rust
use anyhow::Context;

async fn business_operation(
    client: &MessageDbClient,
    account_id: &str,
) -> anyhow::Result<()> {
    let stream_name = format!("account-{}", account_id);

    let messages = client
        .get_stream_messages(StreamReadOptions::new(&stream_name))
        .await
        .context(format!("Failed to read account stream: {}", stream_name))?;

    let balance = calculate_balance(&messages)
        .context("Failed to calculate balance")?;

    println!("Account {} balance: ${}", account_id, balance);
    Ok(())
}
```

## Consumer Error Handling

When using the Consumer pattern:

```rust
// Handle errors in message handlers
consumer.on("Deposited", |msg| {
    Box::pin(async move {
        match process_deposit(&msg).await {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("Failed to process deposit: {}", e);
                // Decide whether to:
                // 1. Return error (consumer will stop)
                // 2. Log and return Ok (skip message)
                // 3. Write to dead letter queue and return Ok

                // For now, return error to stop consumer
                Err(Error::DatabaseError(format!("Processing failed: {}", e)))
            }
        }
    })
});

// Handle errors from consumer polling
match consumer.poll_once().await {
    Ok(had_messages) => {
        println!("Processed batch: {}", had_messages);
    }
    Err(Error::ConnectionError(msg)) => {
        eprintln!("Connection lost: {}, will retry", msg);
        // Implement backoff and retry
    }
    Err(e) => {
        eprintln!("Consumer error: {}", e);
        // Decide whether to stop or continue
    }
}
```

## Best Practices Summary

1. **Always handle errors explicitly**
   - Don't use `.unwrap()` in production code
   - Use `?` operator for propagating errors
   - Match specific error variants when recovery is possible

2. **Distinguish error types**
   - Retry transient errors (connection, concurrency)
   - Don't retry permanent errors (validation)
   - Log all errors with context

3. **Implement retry logic carefully**
   - Use maximum retry limits
   - Implement exponential backoff
   - Re-read state before retrying writes
   - Consider circuit breakers for cascading failures

4. **Add context to errors**
   - Include stream names, IDs, and operation details
   - Use error wrapping (anyhow, thiserror) for better debugging
   - Log errors with structured data

5. **Monitor and alert**
   - Track error rates by type
   - Alert on unexpected error patterns
   - Monitor concurrency conflict rates
   - Watch connection pool exhaustion

6. **Test error scenarios**
   - Test with concurrent operations
   - Simulate connection failures
   - Verify retry logic
   - Test transaction rollback

## Error Logging Example

```rust
use tracing::{error, warn, info};

match client.write_message(event).await {
    Ok(position) => {
        info!(
            stream_name = %stream_name,
            position = position,
            "Message written successfully"
        );
    }
    Err(Error::ConcurrencyError { stream_name, expected_version, actual_version }) => {
        warn!(
            stream_name = %stream_name,
            expected_version = expected_version,
            actual_version = ?actual_version,
            "Concurrency conflict detected"
        );
    }
    Err(Error::ValidationError(msg)) => {
        error!(
            error = %msg,
            stream_name = %stream_name,
            "Validation error"
        );
    }
    Err(e) => {
        error!(
            error = %e,
            stream_name = %stream_name,
            "Unexpected error"
        );
    }
}
```

## Testing Error Handling

```rust
#[tokio::test]
async fn test_concurrency_error_handling() {
    let client = create_test_client().await;
    let stream_name = "test-stream";

    // Write initial event
    let event1 = WriteMessage::new(Uuid::new_v4(), stream_name, "Event1")
        .with_data(json!({}));
    client.write_message(event1).await.unwrap();

    // Try to write with wrong version
    let event2 = WriteMessage::new(Uuid::new_v4(), stream_name, "Event2")
        .with_data(json!({}))
        .with_expected_version(99); // Wrong version

    match client.write_message(event2).await {
        Err(Error::ConcurrencyError { .. }) => {
            // Expected
        }
        result => panic!("Expected ConcurrencyError, got: {:?}", result),
    }
}
```

## Debugging Tips

1. **Enable debug logging** for the Message DB client
2. **Check PostgreSQL logs** for detailed error messages
3. **Verify Message DB functions** exist in the database
4. **Test with Message DB Docker** image for consistency
5. **Use `RUST_BACKTRACE=1`** for detailed stack traces
6. **Monitor connection pool** metrics
7. **Check for schema mismatches** between client and database

## Further Reading

- [Message DB Documentation](https://github.com/message-db/message-db)
- [Optimistic Concurrency Control](MESSAGE_DB_CLIENT_SPEC.md#7-concurrency-control)
- [Transaction Support](MESSAGE_DB_CLIENT_SPEC.md#8-database-transactions)
- [Error Types API Documentation](https://docs.rs/your-crate-name)
