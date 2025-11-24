# Simple Agent Loop Plan

## Overview

This plan outlines the design and implementation of a simple agent loop that manages conversation history, handles LLM interactions, executes tool calls, and continues the conversation until the LLM produces a final text response without tool calls.

## Goal

Create a straightforward, in-memory agent that:
1. Maintains conversation history
2. Accepts new user messages
3. Calls the LLM and streams all responses to the caller
4. Automatically executes tool calls and adds results back to the conversation
5. Loops until the LLM produces a text-only response (no tool calls)
6. Returns a stream of all events (LLM responses, tool executions, etc.) throughout the entire agent loop

## Architecture

### Core Components

#### 1. Agent State
```rust
pub struct Agent {
    /// LLM provider (Claude or Gemini)
    provider: Box<dyn LlmProvider>,

    /// Tool executor for handling function calls
    tool_executor: Box<dyn ToolExecutor>,

    /// Tool declarations available to the LLM
    tool_declarations: Vec<ToolDeclaration>,

    /// Conversation history (kept in memory)
    messages: Vec<Message>,

    /// Generation configuration (temperature, max_tokens, etc.)
    config: GenerationConfig,

    /// System prompt (optional)
    system: Option<String>,

    /// Maximum number of agent loop iterations (default: 10)
    max_iterations: usize,
}
```

**Key Design Decisions:**
- `provider` is boxed to allow dynamic dispatch (can swap Claude/Gemini)
- `tool_executor` is boxed to support different execution strategies
- `messages` vec maintains full conversation history in memory
- `tool_declarations` are registered once during agent construction
- `max_iterations` prevents infinite loops if LLM keeps calling tools

#### 2. Agent Events

The agent emits events to the caller as the loop progresses:

```rust
/// Events emitted by the agent during execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Raw LLM streaming event (text deltas, tool calls, etc.)
    LlmEvent(StreamEvent),

    /// Agent is executing a tool call
    ToolExecutionStarted {
        tool_use_id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Tool execution completed successfully
    ToolExecutionCompleted {
        tool_use_id: String,
        name: String,
        result: String,
    },

    /// Tool execution failed with an error
    ToolExecutionFailed {
        tool_use_id: String,
        name: String,
        error: String,
    },

    /// Agent is starting a new iteration (calling LLM again after tool execution)
    IterationStarted {
        iteration: usize,
    },

    /// Agent loop completed (final response with no tool calls)
    Completed,
}
```

**Event Flow:**
1. `IterationStarted { iteration: 1 }` - First LLM call
2. `LlmEvent(...)` - Stream of events from LLM (text, tool calls, etc.)
3. If tools were called:
   - `ToolExecutionStarted` for each tool
   - `ToolExecutionCompleted` or `ToolExecutionFailed` for each tool
   - `IterationStarted { iteration: 2 }` - Next LLM call
   - `LlmEvent(...)` - More streaming events
4. `Completed` - Final event when no more tools to call

#### 3. Agent API

```rust
impl Agent {
    /// Create a new agent with default settings
    pub fn new(
        provider: Box<dyn LlmProvider>,
        tool_executor: Box<dyn ToolExecutor>,
        tool_declarations: Vec<ToolDeclaration>,
        config: GenerationConfig,
        system: Option<String>,
    ) -> Self;

    /// Set the maximum number of iterations (default: 10)
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Process a new user message through the agent loop
    ///
    /// This is the main entry point. It:
    /// 1. Adds the user message to conversation history
    /// 2. Calls the LLM and streams all events
    /// 3. Executes any tool calls automatically
    /// 4. Loops until getting a text-only response
    /// 5. Returns a stream of all events throughout the entire loop
    ///
    /// The returned stream will emit:
    /// - IterationStarted events when calling the LLM
    /// - LlmEvent events for all streaming responses from the LLM
    /// - ToolExecution* events when executing tools
    /// - Completed event when the agent loop finishes
    pub async fn run(
        &mut self,
        user_message: impl Into<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send>>, AgentError>;

    /// Get the full conversation history
    pub fn messages(&self) -> &[Message];

    /// Clear conversation history (start fresh)
    pub fn clear_history(&mut self);
}
```

### Agent Loop Flow

```
┌──────────────────────────────────────────┐
│ User calls agent.run("user message")     │
│ Returns: Stream<AgentEvent>              │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│ Add user message to history              │
└──────────────┬───────────────────────────┘
               │
               ▼
       ┌───────────────┐
       │  Agent Loop   │◄──────────────────┐
       └───────┬───────┘                   │
               │                           │
               ▼                           │
┌──────────────────────────────────────────┐
│ Emit: IterationStarted                   │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│ Call LLM with current message history    │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│ Stream LLM response:                     │
│ - Emit: LlmEvent for each StreamEvent   │
│ - Accumulate text & tool use blocks     │
└──────────────┬───────────────────────────┘
               │
               ▼
         ┌─────────┐
         │ Has     │ No
         │ tools?  ├──────────────┐
         └────┬────┘              │
              │ Yes               │
              ▼                   ▼
┌──────────────────────┐   ┌──────────────────┐
│ Add assistant msg    │   │ Emit: Completed  │
│ with tool uses       │   │ Stream ends      │
└──────┬───────────────┘   └──────────────────┘
       │
       ▼
┌──────────────────────────────────────────┐
│ For each tool:                           │
│ - Emit: ToolExecutionStarted             │
│ - Execute tool (parallel)                │
│ - Emit: ToolExecutionCompleted/Failed    │
└──────────────┬───────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│ Add tool result messages to history      │
└──────────────┬───────────────────────────┘
               │
               └──────────────────────────────┘
                 (loop continues)
```

### Implementation Details

The implementation uses an async channel to emit events as the agent loop progresses.

#### Overall Structure
```rust
pub async fn run(
    &mut self,
    user_message: impl Into<String>,
) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send>>, AgentError> {
    // Create a channel for sending events
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<AgentEvent, AgentError>>(100);

    // Clone necessary data for the async task
    let provider = self.provider.clone(); // Will need Arc or similar
    let tool_executor = self.tool_executor.clone();
    let tool_declarations = self.tool_declarations.clone();
    let config = self.config.clone();
    let system = self.system.clone();

    // Add user message to history
    self.messages.push(Message::user(user_message));
    let mut messages = self.messages.clone();

    // Spawn task to run the agent loop
    tokio::spawn(async move {
        if let Err(e) = run_agent_loop(
            tx.clone(),
            provider,
            tool_executor,
            tool_declarations,
            config,
            system,
            &mut messages,
        ).await {
            let _ = tx.send(Err(e)).await;
        }
    });

    // Return stream from receiver
    Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
}
```

**Note:** This approach requires `Agent` fields to be wrapped in `Arc` for cloning. Alternative: use `&mut self` and don't spawn a task (simpler but blocks the agent).

#### Simplified Approach (No Spawning)
A simpler approach that doesn't require `Arc` wrapping:

```rust
pub async fn run(
    &mut self,
    user_message: impl Into<String>,
) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send>>, AgentError> {
    // Add user message to history
    self.messages.push(Message::user(user_message));

    // Create the stream using async_stream::stream!
    let stream = self.create_agent_stream().await?;

    Ok(Box::pin(stream))
}

async fn create_agent_stream(&mut self) -> Result<impl Stream<Item = Result<AgentEvent, AgentError>>, AgentError> {
    // Use async_stream crate
    Ok(async_stream::stream! {
        let mut iteration = 0;

        loop {
            iteration += 1;

            // Emit iteration started
            yield Ok(AgentEvent::IterationStarted { iteration });

            // Create LLM request
            let request = GenerateRequest {
                messages: self.messages.clone(),
                tools: Some(self.tool_declarations.clone()),
                config: self.config.clone(),
                system: self.system.clone(),
            };

            // Call LLM and stream response
            let llm_stream = match self.provider.stream_generate(request).await {
                Ok(s) => s,
                Err(e) => {
                    yield Err(AgentError::Llm(e));
                    return;
                }
            };

            // Process LLM stream (see Step 2)
            // ...

            // Check for tools and either complete or continue loop
            // (see Step 3)
        }
    })
}
```

#### Step 1: Stream LLM Events
```rust
// Accumulate response data while forwarding events
let mut text_content = String::new();
let mut tool_uses = Vec::new();
let mut current_tool_use: Option<PartialToolUse> = None;

pin_mut!(llm_stream);  // Pin the stream for iteration

while let Some(event_result) = llm_stream.next().await {
    let event = match event_result {
        Ok(e) => e,
        Err(e) => {
            yield Err(AgentError::Llm(e));
            return;
        }
    };

    // Forward the LLM event to caller
    yield Ok(AgentEvent::LlmEvent(event.clone()));

    // Also accumulate data for tool detection
    match event {
        StreamEvent::ContentBlockStart { block, .. } => {
            match block {
                ContentBlockStart::Text { text } => {
                    text_content.push_str(&text);
                }
                ContentBlockStart::ToolUse { id, name } => {
                    current_tool_use = Some(PartialToolUse {
                        id,
                        name,
                        input: String::new(),
                    });
                }
            }
        }
        StreamEvent::ContentDelta { delta, .. } => {
            match delta {
                ContentDelta::TextDelta { text } => {
                    text_content.push_str(&text);
                }
                ContentDelta::ToolUseDelta { partial } => {
                    if let Some(tool_use) = &mut current_tool_use {
                        tool_use.input.push_str(&partial.input);
                    }
                }
            }
        }
        StreamEvent::ContentBlockEnd { .. } => {
            if let Some(tool_use) = current_tool_use.take() {
                // Parse complete tool use
                match serde_json::from_str(&tool_use.input) {
                    Ok(input) => {
                        tool_uses.push(ContentBlock::ToolUse {
                            id: tool_use.id,
                            name: tool_use.name,
                            input,
                        });
                    }
                    Err(e) => {
                        yield Err(AgentError::ToolInputParse(e));
                        return;
                    }
                }
            }
        }
        StreamEvent::MessageEnd { .. } => break,
        _ => {}
    }
}
```

#### Step 2: Tool Execution or Completion
```rust
// Check if we need to execute tools
if tool_uses.is_empty() {
    // No tools - we're done!
    yield Ok(AgentEvent::Completed);
    return;
}

// Build assistant message with tool uses
let mut assistant_content = Vec::new();
if !text_content.is_empty() {
    assistant_content.push(ContentBlock::Text { text: text_content });
}
assistant_content.extend(tool_uses.clone());

// Add to conversation history
self.messages.push(Message {
    role: MessageRole::Assistant,
    content: assistant_content,
});

// Execute tools and emit events
for block in &tool_uses {
    if let ContentBlock::ToolUse { id, name, input } = block {
        // Emit tool execution started
        yield Ok(AgentEvent::ToolExecutionStarted {
            tool_use_id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        });

        // Execute the tool
        match self.tool_executor.execute(
            id.clone(),
            name.clone(),
            input.clone(),
        ).await {
            Ok(result) => {
                yield Ok(AgentEvent::ToolExecutionCompleted {
                    tool_use_id: id.clone(),
                    name: name.clone(),
                    result: result.clone(),
                });

                // Add tool result to history
                self.messages.push(Message::tool_result(id.clone(), result));
            }
            Err(error) => {
                yield Ok(AgentEvent::ToolExecutionFailed {
                    tool_use_id: id.clone(),
                    name: name.clone(),
                    error: error.clone(),
                });

                // Add tool error to history
                self.messages.push(Message::tool_error(id.clone(), error));
            }
        }
    }
}

// Check max iterations
if iteration >= self.max_iterations {
    yield Err(AgentError::MaxIterationsReached(iteration));
    return;
}

// Loop continues - next iteration will call LLM again
```

**Note:** The example above shows sequential tool execution for clarity. For parallel execution, you would collect futures and use `join_all`, but this complicates event emission. Consider if parallel execution is needed or if sequential is acceptable.

## Error Handling

### Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Failed to parse tool input: {0}")]
    ToolInputParse(#[from] serde_json::Error),

    #[error("Stream ended unexpectedly")]
    UnexpectedStreamEnd,

    #[error("Maximum iterations reached ({0})")]
    MaxIterationsReached(usize),
}
```

### Safety Mechanisms
- **Max iterations**: Prevent infinite loops if LLM keeps calling tools
  - Default: 10 iterations
  - Configurable via `Agent::with_max_iterations()`
- **Stream errors**: Propagate LLM errors immediately
- **Tool errors**: Don't fail the agent - return error as tool result for LLM to handle

## Usage Example

```rust
use rust2::llm::{
    create_provider, ClaudeModel, Model, GenerationConfig, ToolDeclaration,
    StreamEvent, ContentDelta,
};
use rust2::llm::tools::FunctionRegistry;
use rust2::llm::agent::{Agent, AgentEvent};
use serde::{Deserialize, Serialize};
use futures::StreamExt;
use std::io::Write;

#[derive(Deserialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

#[derive(Serialize)]
struct CalculatorResult {
    result: f64,
}

async fn calculator(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    let result = match args.operation.as_str() {
        "add" => args.a + args.b,
        "subtract" => args.a - args.b,
        "multiply" => args.a * args.b,
        "divide" if args.b != 0.0 => args.a / args.b,
        "divide" => return Err("Division by zero".to_string()),
        _ => return Err(format!("Unknown operation: {}", args.operation)),
    };

    Ok(CalculatorResult { result })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up LLM provider
    let provider = create_provider(
        Model::Claude(ClaudeModel::Haiku45),
        "my-project".to_string(),
        "us-central1".to_string(),
    ).await?;

    // Set up tools
    let mut registry = FunctionRegistry::new();
    registry.register_async("calculator", calculator);

    let tool_declarations = vec![
        ToolDeclaration {
            name: "calculator".to_string(),
            description: "Perform basic arithmetic operations".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide"],
                        "description": "The operation to perform"
                    },
                    "a": {
                        "type": "number",
                        "description": "First operand"
                    },
                    "b": {
                        "type": "number",
                        "description": "Second operand"
                    }
                },
                "required": ["operation", "a", "b"]
            }),
        }
    ];

    // Create agent
    let mut agent = Agent::new(
        provider,
        Box::new(registry),
        tool_declarations,
        GenerationConfig::new(1024).with_temperature(0.7),
        Some("You are a helpful assistant with access to a calculator.".to_string()),
    );

    // Run conversation and stream events
    println!("User: What is 15 multiplied by 23?\n");

    let mut stream = agent.run("What is 15 multiplied by 23?").await?;
    let mut final_text = String::new();

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::IterationStarted { iteration } => {
                println!("[Iteration {}]", iteration);
            }
            AgentEvent::LlmEvent(StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text },
                ..
            }) => {
                print!("{}", text);
                final_text.push_str(&text);
                std::io::stdout().flush()?;
            }
            AgentEvent::ToolExecutionStarted { name, input, .. } => {
                println!("\n[Calling tool: {} with args: {}]", name, input);
            }
            AgentEvent::ToolExecutionCompleted { name, result, .. } => {
                println!("[Tool {} completed: {}]", name, result);
            }
            AgentEvent::ToolExecutionFailed { name, error, .. } => {
                println!("[Tool {} failed: {}]", name, error);
            }
            AgentEvent::Completed => {
                println!("\n[Agent completed]\n");
            }
            _ => {}
        }
    }

    // Continue conversation with history
    println!("User: Now divide that by 5\n");

    let mut stream = agent.run("Now divide that by 5").await?;

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::IterationStarted { iteration } => {
                println!("[Iteration {}]", iteration);
            }
            AgentEvent::LlmEvent(StreamEvent::ContentDelta {
                delta: ContentDelta::TextDelta { text },
                ..
            }) => {
                print!("{}", text);
                std::io::stdout().flush()?;
            }
            AgentEvent::ToolExecutionStarted { name, input, .. } => {
                println!("\n[Calling tool: {} with args: {}]", name, input);
            }
            AgentEvent::ToolExecutionCompleted { name, result, .. } => {
                println!("[Tool {} completed: {}]", name, result);
            }
            AgentEvent::Completed => {
                println!("\n[Agent completed]\n");
            }
            _ => {}
        }
    }

    // Check conversation history
    println!("\nFull conversation:");
    for (i, msg) in agent.messages().iter().enumerate() {
        println!("{}. {:?}", i + 1, msg.role);
    }

    Ok(())
}
```

**Expected Output:**
```
User: What is 15 multiplied by 23?

[Iteration 1]
[Calling tool: calculator with args: {"operation":"multiply","a":15.0,"b":23.0}]
[Tool calculator completed: {"result":345.0}]
[Iteration 2]
15 multiplied by 23 equals 345.
[Agent completed]

User: Now divide that by 5

[Iteration 1]
[Calling tool: calculator with args: {"operation":"divide","a":345.0,"b":5.0}]
[Tool calculator completed: {"result":69.0}]
[Iteration 2]
345 divided by 5 equals 69.
[Agent completed]

Full conversation:
1. User
2. Assistant
3. Tool
4. Assistant
5. User
6. Assistant
7. Tool
8. Assistant
```

## Implementation Checklist

### Phase 1: Core Agent Structure
- [ ] Create `src/llm/agent/mod.rs`
- [ ] Define `Agent` struct
- [ ] Define `AgentError` enum
- [ ] Implement `Agent::new()`
- [ ] Implement `Agent::messages()` and `Agent::clear_history()`

### Phase 2: Agent Event Types
- [ ] Define `AgentEvent` enum
- [ ] Define event variants (LlmEvent, ToolExecution*, IterationStarted, Completed)

### Phase 3: Main Agent Loop with Streaming
- [ ] Implement `Agent::run()` method that returns a stream
- [ ] Add user message to history
- [ ] Set up async stream using `async_stream` crate
- [ ] Emit `IterationStarted` event at start of each iteration
- [ ] Create LLM request with current conversation state
- [ ] Forward LLM events as `AgentEvent::LlmEvent`
- [ ] Accumulate text and tool uses while streaming
- [ ] Implement iteration counter and max iterations check

### Phase 4: Tool Execution Integration
- [ ] Build assistant message with tool use blocks
- [ ] Emit `ToolExecutionStarted` events before executing tools
- [ ] Execute tools (sequential or parallel)
- [ ] Emit `ToolExecutionCompleted` or `ToolExecutionFailed` events
- [ ] Handle tool execution errors gracefully
- [ ] Add tool result messages to history
- [ ] Loop back to LLM call with updated history

### Phase 5: Response Processing
- [ ] Extract text content from streaming events
- [ ] Extract tool use blocks from streaming events
- [ ] Handle both ContentBlockStart and ContentDelta events
- [ ] Properly accumulate partial tool inputs
- [ ] Parse complete tool input JSON
- [ ] Emit `Completed` event when no tools are called

### Phase 6: Testing
- [ ] Unit test: Agent creation
- [ ] Unit test: Message history management
- [ ] Unit test: Event emission order
- [ ] Integration test: Simple text conversation (no tools)
- [ ] Integration test: Single tool call
- [ ] Integration test: Multiple tool calls in one response
- [ ] Integration test: Multi-turn conversation with tools
- [ ] Integration test: Tool error handling
- [ ] Integration test: Max iterations limit
- [ ] Integration test: Verify all events are emitted correctly

### Phase 7: Documentation & Examples
- [ ] Add rustdoc comments to all public APIs
- [ ] Create `examples/agent_simple.rs` - basic agent loop
- [ ] Create `examples/agent_calculator.rs` - math agent with tools
- [ ] Create `examples/agent_multi_turn.rs` - multi-turn conversation

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies ...

# For creating async streams easily
async-stream = "0.3"

# For wrapping channels as streams
tokio-stream = { version = "0.1", features = ["sync"] }

# For pinning streams
futures = "0.3"  # Already in dependencies
```

## File Structure

```
src/llm/
├── agent/
│   ├── mod.rs           # Agent struct, AgentEvent enum, and main loop implementation
│   └── error.rs         # AgentError types
├── core/
│   └── ...              # Existing core types
├── tools/
│   └── ...              # Existing tool executor/registry
└── mod.rs               # Export agent module

examples/
├── llm/
│   ├── agent_simple.rs      # Basic agent example
│   ├── agent_calculator.rs  # Math agent with calculator tool
│   └── agent_multi_turn.rs  # Multi-turn conversation example
└── ...
```

## Advanced Features (Future Enhancements)

These are out of scope for the initial simple implementation but could be added later:

### 1. Tool Call Interception/Approval
Allow caller to approve tool calls before execution (for sensitive operations).

```rust
pub trait ToolApprover {
    async fn approve_tool_call(&self, name: &str, args: &serde_json::Value) -> bool;
}
```

### 2. Conversation Persistence
Save/load conversation history to/from disk or database.

```rust
pub async fn save_to_file(&self, path: &Path) -> Result<(), std::io::Error>;
pub async fn load_from_file(path: &Path) -> Result<Vec<Message>, std::io::Error>;
```

### 3. Message History Limits
Automatically truncate old messages to stay within token limits.

```rust
pub fn with_max_history_tokens(self, max_tokens: usize) -> Self;
```

### 4. Parallel Tool Execution
Execute multiple tools in parallel instead of sequentially, with coordinated event emission.

### 5. Simplified Stream Helper
Provide a helper method to collect just the final text response for simple use cases:

```rust
pub async fn run_simple(&mut self, user_message: impl Into<String>) -> Result<String, AgentError>;
```

## Implementation Considerations

### Borrowing Challenges with Async Streams

The `async_stream::stream!` macro creates a stream that captures its environment. This can create borrowing challenges when trying to use `&mut self` inside the stream. There are two approaches:

**Approach 1: Move ownership into stream (Recommended)**
- Move all necessary state into the stream
- Update `self.messages` after the stream completes
- Simpler to implement but requires cloning state

**Approach 2: Use channels and spawn**
- Spawn a task that runs the agent loop
- Send events through a channel
- Wrap channel receiver as a stream
- More complex but avoids borrowing issues

For the initial implementation, Approach 1 is recommended for simplicity. The agent loop can clone the current state, run the stream, and then update `self.messages` based on what happened.

### StreamEvent Cloning

The `AgentEvent::LlmEvent` variant needs to clone `StreamEvent` to both forward it to the caller and process it internally. This requires `StreamEvent` to implement `Clone`. Check if it already does, or add `#[derive(Clone)]` if needed.

### Max Iterations Handling

The agent should track iteration count and return an error if it exceeds the limit. This prevents infinite loops if the LLM keeps requesting tool calls. A reasonable default is 10 iterations, configurable via a builder method or constructor parameter.

### Updating Agent State After Stream

Since the stream owns a mutable reference to `self`, it can directly update `self.messages` as it processes the conversation. This means:
- When the LLM responds with text and/or tool uses, append to `self.messages` immediately
- When tools are executed, append tool result messages immediately
- By the time the stream ends, `self.messages` is fully up to date
- This allows subsequent calls to `agent.run()` to use the updated history

This is the key advantage of not spawning a separate task - the stream has direct mutable access to the agent's state.

## Success Criteria

The implementation will be considered successful when:

1. ✅ Agent can maintain conversation history across multiple turns
2. ✅ Agent can process user messages and call LLM
3. ✅ Agent returns a stream of `AgentEvent`s to the caller
4. ✅ Agent emits `LlmEvent`s for all streaming responses from the LLM
5. ✅ Agent can detect tool calls in LLM responses
6. ✅ Agent emits `ToolExecutionStarted` events before executing tools
7. ✅ Agent can execute tools using the ToolExecutor trait
8. ✅ Agent emits `ToolExecutionCompleted` or `ToolExecutionFailed` events
9. ✅ Agent can add tool results back to conversation and continue
10. ✅ Agent loops until getting a text-only response
11. ✅ Agent emits `Completed` event when the loop finishes
12. ✅ Agent handles tool execution errors gracefully
13. ✅ Agent prevents infinite loops with max iterations limit
14. ✅ All integration tests pass with both Claude and Gemini providers
15. ✅ Event ordering is correct and predictable
