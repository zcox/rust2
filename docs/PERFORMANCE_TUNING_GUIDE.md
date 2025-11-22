# Performance Tuning Guide

This guide provides recommendations for optimizing Message DB Rust client performance in production environments.

## Table of Contents

1. [Connection Pool Configuration](#connection-pool-configuration)
2. [Batch Size Optimization](#batch-size-optimization)
3. [Position Update Strategy](#position-update-strategy)
4. [Consumer Patterns](#consumer-patterns)
5. [Transaction Best Practices](#transaction-best-practices)
6. [Query Optimization](#query-optimization)
7. [Network and I/O](#network-and-io)
8. [Monitoring and Metrics](#monitoring-and-metrics)

## Connection Pool Configuration

### Pool Size

The connection pool size should be tuned based on your workload:

```rust
let config = MessageDbConfig::from_connection_string(
    "postgresql://postgres:password@localhost:5432/message_store"
)?
.with_max_pool_size(20)  // Adjust based on workload
.with_connect_timeout(Duration::from_secs(30));
```

**Recommendations:**

| Workload Type | Recommended Pool Size |
|---------------|----------------------|
| Low throughput (< 100 msg/s) | 5-10 connections |
| Medium throughput (100-1000 msg/s) | 10-20 connections |
| High throughput (> 1000 msg/s) | 20-50 connections |
| Consumer-heavy | 2-5 per consumer |

**Guidelines:**
- Start with 10 connections and monitor
- Each consumer needs 1-2 connections
- Too many connections waste resources
- Too few connections cause bottlenecks
- Monitor pool saturation metrics

### Connection Timeout

```rust
let config = config.with_connect_timeout(Duration::from_secs(30));
```

- Default: 30 seconds (recommended)
- Increase for unreliable networks
- Decrease for faster failure detection
- Consider network latency to database

### Connection Recycling

deadpool-postgres recycles connections efficiently:

```rust
// No special configuration needed - deadpool handles recycling automatically
// Connections are validated before reuse
```

**Best practices:**
- Don't close/reopen clients frequently
- Reuse `MessageDbClient` instance across application
- Let the pool manage connection lifecycle

## Batch Size Optimization

### Reading Messages

Batch size affects memory usage and round trips:

```rust
// Small batches - more round trips, less memory
let options = StreamReadOptions::new(stream_name)
    .with_batch_size(10);

// Large batches - fewer round trips, more memory
let options = StreamReadOptions::new(stream_name)
    .with_batch_size(1000);
```

**Recommendations:**

| Message Size | Recommended Batch Size |
|--------------|----------------------|
| Small (< 1KB) | 500-1000 |
| Medium (1KB-10KB) | 100-500 |
| Large (> 10KB) | 10-100 |

**Guidelines:**
- Default 1000 works well for most cases
- Reduce for large message payloads
- Increase for small messages
- Monitor memory usage
- Consider network bandwidth

### Consumer Polling

```rust
let consumer_config = ConsumerConfig::new("account", "worker-1")
    .with_batch_size(100)  // Messages per poll
    .with_polling_interval_ms(100);  // Milliseconds between polls
```

**Polling interval recommendations:**
- High throughput: 10-50ms
- Medium throughput: 100-500ms
- Low throughput: 1000-5000ms
- Balance latency vs CPU usage

## Position Update Strategy

### Update Frequency

Position updates trade off between resumability and write overhead:

```rust
let consumer_config = ConsumerConfig::new("account", "worker-1")
    .with_position_update_interval(100);  // Update every 100 messages
```

**Recommendations:**

| Requirement | Update Interval |
|-------------|----------------|
| Maximum resumability | 1 (every message) |
| Balanced | 10-100 messages |
| High throughput | 100-1000 messages |
| Batch processing | After each batch |

**Trade-offs:**
- Frequent updates: Better resumability, higher write load
- Infrequent updates: Better performance, more replay on restart
- Consider message processing cost
- Consider criticality of exactly-once

### Position Storage

**Stream-based (default):**
```rust
// Position stored as messages in position stream
// Pro: Uses Message DB's native model
// Con: Requires read to get latest position
```

**Considerations:**
- Stream-based is standard and recommended
- Position streams grow over time (compaction possible)
- Read performance acceptable for most use cases

## Consumer Patterns

### Consumer Groups

Distribute load across multiple consumers:

```rust
// 3 workers processing in parallel
for member_id in 0..3 {
    let config = ConsumerConfig::new("account", &format!("worker-{}", member_id))
        .with_consumer_group(member_id as i64, 3);

    let consumer = Consumer::new(client.clone(), config).await?;
    // Spawn on separate task
    tokio::spawn(async move {
        consumer.start().await
    });
}
```

**Best practices:**
- Scale horizontally by increasing group size
- Each stream processed by exactly one member
- Consistent hash ensures same stream → same member
- Rebalancing requires restart (not automatic)

### Parallel Processing

Process messages in parallel within a consumer:

```rust
consumer.on("Deposited", |msg| {
    Box::pin(async move {
        // Spawn processing on separate task
        tokio::spawn(async move {
            expensive_operation(&msg).await
        }).await.unwrap()
    })
});
```

**Caution:**
- Breaks ordering guarantees within stream
- Only use if order doesn't matter
- Be careful with position updates
- Consider idempotency requirements

### Handler Optimization

Optimize message handlers:

```rust
consumer.on("Deposited", |msg| {
    Box::pin(async move {
        // Fast path: quick validation
        if !is_valid(&msg) {
            return Ok(()); // Skip invalid messages
        }

        // Batch database operations
        process_batch(vec![msg]).await
    })
});
```

**Tips:**
- Keep handlers fast
- Batch external API calls
- Use async I/O throughout
- Avoid blocking operations
- Consider circuit breakers for external services

## Transaction Best Practices

### Keep Transactions Short

```rust
// GOOD: Short transaction
let mut txn = client.begin_transaction().await?;
txn.write_message(msg1).await?;
txn.write_message(msg2).await?;
txn.commit().await?;

// BAD: Long transaction with I/O
let mut txn = client.begin_transaction().await?;
txn.write_message(msg1).await?;
let data = call_external_api().await?;  // DON'T DO THIS
txn.write_message(msg2).await?;
txn.commit().await?;
```

**Guidelines:**
- Minimize transaction duration
- No external I/O in transactions
- No user input waiting
- No long computations
- Reduces lock contention

### Batch Writes

Use transactions to batch multiple writes:

```rust
let mut txn = client.begin_transaction().await?;

for event in events {
    txn.write_message(event).await?;
}

txn.commit().await?;
```

**Benefits:**
- Atomic writes
- Reduced overhead
- Better throughput
- Lower latency per message

### Transaction Pooling

Reuse connections for transactions:

```rust
// Connection pool handles this automatically
// Just call begin_transaction when needed
let mut txn = client.begin_transaction().await?;
```

## Query Optimization

### Stream Queries

```rust
// Efficient: Read only what you need
let options = StreamReadOptions::new(stream_name)
    .with_position(last_position)  // Start from known position
    .with_batch_size(100);

// Inefficient: Reading entire large stream repeatedly
let all_messages = client.get_stream_messages(
    StreamReadOptions::new(stream_name).with_batch_size(10000)
).await?;
```

### Category Queries

```rust
// Efficient: Use consumer groups for parallel processing
let options = CategoryReadOptions::new("account")
    .with_consumer_group(member_id, group_size)
    .with_batch_size(100);

// Efficient: Start from known position
let options = CategoryReadOptions::new("account")
    .with_position(last_global_position);

// Inefficient: Repeatedly reading from position 1
```

### Condition Filtering

Use SQL conditions to filter at database:

```rust
// GOOD: Filter at database
let options = StreamReadOptions::new(stream_name)
    .with_condition("type = 'Withdrawn'");

// Less efficient: Filter in application
let messages = client.get_stream_messages(options).await?;
let filtered: Vec<_> = messages.iter()
    .filter(|m| m.message_type == "Withdrawn")
    .collect();
```

## Network and I/O

### Connection Location

- **Same datacenter**: < 1ms latency
- **Same region**: 1-10ms latency
- **Cross-region**: 10-100ms+ latency

**Recommendations:**
- Colocate application and database
- Use same cloud region
- Minimize network hops
- Consider read replicas for geo-distribution

### Message Size

Control message payload size:

```rust
// Keep data focused
let event = WriteMessage::new(id, stream, "Withdrawn")
    .with_data(json!({
        "amount": 100,
        "currency": "USD"
        // Don't include large fields unless necessary
    }));

// Avoid large payloads
// Consider references to external storage for large data
```

**Guidelines:**
- Keep messages < 10KB when possible
- Use references for large payloads
- JSONB compression is automatic
- Monitor message size distribution

### Async Operations

Use async throughout:

```rust
// GOOD: Async all the way
async fn process_event(msg: &Message) -> Result<()> {
    let result = async_operation(&msg.data).await?;
    save_result(result).await?;
    Ok(())
}

// BAD: Blocking operations
fn process_event_blocking(msg: &Message) -> Result<()> {
    let result = blocking_operation(&msg.data)?;  // Blocks thread
    save_result_blocking(result)?;
    Ok(())
}
```

## Monitoring and Metrics

### Key Metrics to Track

**Throughput:**
```rust
// Messages written per second
let write_rate = messages_written / elapsed_seconds;

// Messages read per second
let read_rate = messages_read / elapsed_seconds;
```

**Latency:**
```rust
// Write latency (p50, p95, p99)
let start = Instant::now();
client.write_message(msg).await?;
let write_latency = start.elapsed();

// Consumer lag
let consumer_lag = latest_global_position - consumer_position;
```

**Errors:**
```rust
// Concurrency error rate
let concurrency_rate = concurrency_errors / total_writes;

// Connection errors
let connection_error_count;
```

**Connection Pool:**
```rust
// Pool saturation
let pool_saturation = active_connections / max_pool_size;

// Wait time for connection
let connection_wait_time;
```

### Instrumentation Example

```rust
use tracing::{info, instrument};
use std::time::Instant;

#[instrument(skip(client))]
async fn write_with_metrics(
    client: &MessageDbClient,
    msg: WriteMessage,
) -> Result<i64> {
    let start = Instant::now();

    let result = client.write_message(msg).await;

    let duration = start.elapsed();
    info!(
        duration_ms = duration.as_millis(),
        success = result.is_ok(),
        "write_message completed"
    );

    result
}
```

### Prometheus Metrics

```rust
use prometheus::{Counter, Histogram, register_counter, register_histogram};

lazy_static! {
    static ref WRITES_TOTAL: Counter = register_counter!(
        "messagedb_writes_total",
        "Total number of messages written"
    ).unwrap();

    static ref WRITE_DURATION: Histogram = register_histogram!(
        "messagedb_write_duration_seconds",
        "Message write duration"
    ).unwrap();
}

async fn instrumented_write(client: &MessageDbClient, msg: WriteMessage) -> Result<i64> {
    let timer = WRITE_DURATION.start_timer();
    let result = client.write_message(msg).await;
    timer.observe_duration();

    if result.is_ok() {
        WRITES_TOTAL.inc();
    }

    result
}
```

## Performance Checklist

- [ ] Connection pool size tuned for workload
- [ ] Batch sizes optimized for message size
- [ ] Position update interval balanced
- [ ] Consumer groups for horizontal scaling
- [ ] Transactions kept short
- [ ] Database and app colocated
- [ ] Monitoring and metrics in place
- [ ] Async operations throughout
- [ ] Error rates monitored
- [ ] Consumer lag monitored

## Benchmarking

### Simple Benchmark

```rust
use std::time::Instant;

async fn benchmark_writes(client: &MessageDbClient, num_messages: usize) {
    let start = Instant::now();

    for i in 0..num_messages {
        let msg = WriteMessage::new(
            Uuid::new_v4(),
            "benchmark-stream",
            "BenchmarkEvent"
        ).with_data(json!({ "seq": i }));

        client.write_message(msg).await.unwrap();
    }

    let duration = start.elapsed();
    let rate = num_messages as f64 / duration.as_secs_f64();

    println!("Wrote {} messages in {:?}", num_messages, duration);
    println!("Rate: {:.2} messages/second", rate);
}
```

### Load Testing

Use tools like:
- `ab` (Apache Bench)
- `wrk`
- `k6`
- Custom Rust benchmarks with `criterion`

## Common Performance Issues

### Issue: High Consumer Lag

**Symptoms:**
- Consumer position far behind latest messages
- Growing lag over time

**Solutions:**
- Increase consumer group size
- Optimize message handlers
- Increase batch size
- Reduce position update frequency
- Check for slow handlers

### Issue: Connection Pool Exhaustion

**Symptoms:**
- Timeouts acquiring connections
- High wait times

**Solutions:**
- Increase pool size
- Reduce connection hold time
- Check for connection leaks
- Optimize transaction duration

### Issue: Slow Writes

**Symptoms:**
- High write latency
- Low write throughput

**Solutions:**
- Use transactions for batching
- Check network latency
- Verify database performance
- Check for lock contention
- Monitor disk I/O on database

### Issue: High Memory Usage

**Symptoms:**
- Growing memory consumption
- Out of memory errors

**Solutions:**
- Reduce batch sizes
- Process messages in smaller chunks
- Don't hold messages in memory
- Stream large result sets
- Check for memory leaks

## Advanced Optimizations

### Statement Caching

tokio-postgres caches prepared statements automatically:

```rust
// Statements are automatically prepared and cached
// No special configuration needed
```

### Query Pipelining

tokio-postgres supports query pipelining:

```rust
// Future optimization: pipeline multiple queries
// Currently not exposed in client API
// Could provide 10-20% performance boost
```

### Read Replicas

For read-heavy workloads:

```rust
// Configure separate client for reads
let read_config = MessageDbConfig::from_connection_string(
    "postgresql://readonly@read-replica:5432/message_store"
)?;
let read_client = MessageDbClient::new(read_config).await?;

// Use for read operations
let messages = read_client.get_stream_messages(options).await?;

// Use write client for writes
write_client.write_message(msg).await?;
```

## Further Reading

- [PostgreSQL Performance Tips](https://www.postgresql.org/docs/current/performance-tips.html)
- [tokio-postgres Documentation](https://docs.rs/tokio-postgres)
- [deadpool Documentation](https://docs.rs/deadpool-postgres)
- [Message DB Performance](https://github.com/message-db/message-db)

## Profiling Tools

- **Rust**: `cargo flamegraph`, `perf`, `valgrind`
- **PostgreSQL**: `pg_stat_statements`, `EXPLAIN ANALYZE`
- **Network**: `tcpdump`, `Wireshark`
- **System**: `htop`, `iotop`, `vmstat`
