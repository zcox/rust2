//! Event-sourced agent implementation
//!
//! This module provides an `EventSourcedAgent` that persists all conversation
//! history and agent execution state to MessageDB using event sourcing.
//!
//! Key features:
//! - All events stored in MessageDB streams
//! - Can reconstruct conversation from events
//! - Supports resuming threads
//! - Emits AgentEvents for real-time streaming
//! - Optimistic concurrency control

use super::events::{
    AgentCompletedData, AgentEvent, AgentFailedData, AgentIterationCompletedData,
    AgentIterationStartedData, ContentBlockData, LlmCallStartedData, LlmContentDeltaData,
    LlmResponseCompletedData, LlmToolUseCompletedData, LlmToolUseDeltaData,
    LlmToolUseStartedData, ThreadEvent, ToolExecutionCompletedData, ToolExecutionFailedData,
    ToolExecutionStartedData, UserMessageReceivedData,
};
use super::projection::project_events_to_messages;
use super::store::ThreadStore;
use super::AgentError;
use crate::llm::core::{
    config::GenerationConfig,
    provider::LlmProvider,
    types::{
        ContentBlock, ContentBlockStart, ContentDelta, GenerateRequest, Message, MessageRole,
        StreamEvent, ToolDeclaration,
    },
};
use crate::llm::tools::executor::ToolExecutor;
use chrono::Utc;
use futures::stream::Stream;
use futures::StreamExt;
use pin_utils::pin_mut;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Helper struct for accumulating partial tool use data
struct PartialToolUseAccumulator {
    id: String,
    name: String,
    input: String,
}

/// Event-sourced agent that persists to MessageDB
///
/// This agent stores all conversation and execution events in MessageDB,
/// allowing for full auditability, replay, and resumption of conversations.
///
/// # Example
///
/// ```no_run
/// use rust2::llm::agent::EventSourcedAgent;
/// use rust2::llm::core::config::GenerationConfig;
/// use rust2::message_db::{MessageDbClient, MessageDbConfig};
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Set up dependencies (provider, executor, store)
///     # let provider: Box<dyn rust2::llm::core::provider::LlmProvider> = todo!();
///     # let executor: Box<dyn rust2::llm::tools::executor::ToolExecutor> = todo!();
///     # let config = MessageDbConfig::from_connection_string("postgresql://postgres:password@localhost:5433/message_store")?;
///     # let client = MessageDbClient::new(config).await?;
///     # let store = rust2::llm::agent::ThreadStore::new(client);
///
///     // Create event-sourced agent
///     let agent = EventSourcedAgent::new(
///         provider,
///         executor,
///         store,
///         vec![],
///         GenerationConfig::new(1024),
///         None,
///     );
///
///     // Run agent and stream events
///     let mut stream = agent.run("thread-123".to_string(), "What's the weather?".to_string()).await?;
///
///     use futures::StreamExt;
///     while let Some(event) = stream.next().await {
///         match event? {
///             rust2::llm::agent::events::AgentEvent::TextDelta(text) => print!("{}", text),
///             rust2::llm::agent::events::AgentEvent::Completed => println!("\nDone!"),
///             _ => {}
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub struct EventSourcedAgent {
    /// LLM provider (Claude or Gemini)
    provider: std::sync::Arc<dyn LlmProvider>,

    /// Tool executor for handling function calls
    tool_executor: std::sync::Arc<dyn ToolExecutor>,

    /// Thread store for MessageDB persistence
    store: ThreadStore,

    /// Tool declarations available to the LLM
    tool_declarations: Vec<ToolDeclaration>,

    /// Generation configuration (temperature, max_tokens, etc.)
    config: GenerationConfig,

    /// System prompt (optional)
    system: Option<String>,

    /// Maximum number of agent loop iterations (default: 10)
    max_iterations: usize,
}

impl EventSourcedAgent {
    /// Create a new event-sourced agent
    pub fn new(
        provider: std::sync::Arc<dyn LlmProvider>,
        tool_executor: std::sync::Arc<dyn ToolExecutor>,
        store: ThreadStore,
        tool_declarations: Vec<ToolDeclaration>,
        config: GenerationConfig,
        system: Option<String>,
    ) -> Self {
        Self {
            provider,
            tool_executor,
            store,
            tool_declarations,
            config,
            system,
            max_iterations: 10,
        }
    }

    /// Set the maximum number of iterations (default: 10)
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Process a new user message through the event-sourced agent loop
    ///
    /// This is the main entry point. It:
    /// 1. Appends UserMessageReceived event to the thread stream
    /// 2. Reads all thread events and projects them to messages
    /// 3. Calls the LLM and streams all events
    /// 4. Persists all events to MessageDB
    /// 5. Executes any tool calls automatically
    /// 6. Loops until getting a text-only response
    /// 7. Returns a stream of AgentEvent for real-time updates
    ///
    /// The returned stream will emit:
    /// - UserMessage when the message is received
    /// - IterationStarted events when calling the LLM
    /// - TextDelta events for streaming text from the LLM
    /// - ToolExecution* events when executing tools
    /// - Completed event when the agent loop finishes
    ///
    /// All events are also persisted to MessageDB for auditability and replay.
    pub async fn run(
        &self,
        thread_id: String,
        user_message: String,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send + Sync>>, AgentError>
    {
        // Create a channel for streaming events
        let (tx, rx) = mpsc::channel::<Result<AgentEvent, AgentError>>(100);

        // Clone what we need for the async task
        let provider = self.provider.clone();
        let tool_executor = self.tool_executor.clone();
        let store = self.store.clone();
        let tool_declarations = self.tool_declarations.clone();
        let config = self.config.clone();
        let system = self.system.clone();
        let max_iterations = self.max_iterations;

        // Spawn a task to run the agent loop and send events to the channel
        tokio::spawn(async move {
            let now = Utc::now();

            // 1. Append UserMessageReceived event
            let user_event = ThreadEvent::UserMessageReceived(UserMessageReceivedData {
                message: user_message.clone(),
                timestamp: now,
            });

            if let Err(e) = store.append_event(&thread_id, user_event.clone(), None, None).await {
                let _ = tx.send(Err(AgentError::Store(format!("Failed to append user message: {}", e)))).await;
                return;
            }

            // Emit user message event
            if let Ok(agent_event) = AgentEvent::try_from(user_event) {
                let _ = tx.send(Ok(agent_event)).await;
            }

            // 2. Read all thread events and project to messages
            let events = match store.read_thread_events(&thread_id).await {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(Err(AgentError::Store(format!("Failed to read thread events: {}", e)))).await;
                    return;
                }
            };

            let mut messages = project_events_to_messages(&events);

            // 3. Enter agent loop
            let mut iteration = 0;

            loop {
                iteration += 1;

                // Check max iterations
                if iteration > max_iterations {
                    let failed_event = ThreadEvent::AgentFailed(AgentFailedData {
                        error: "MaxIterationsReached".to_string(),
                        details: format!("Exceeded maximum of {} iterations", max_iterations),
                        iteration,
                        timestamp: Utc::now(),
                    });

                    let _ = store.append_event(&thread_id, failed_event, None, None).await;
                    let _ = tx.send(Err(AgentError::MaxIterationsReached(iteration - 1))).await;
                    return;
                }

                // Emit iteration started event
                let iteration_event = ThreadEvent::AgentIterationStarted(AgentIterationStartedData {
                    iteration,
                    timestamp: Utc::now(),
                });

                if let Err(e) = store.append_event(&thread_id, iteration_event.clone(), None, None).await {
                    let _ = tx.send(Err(AgentError::Store(format!("Failed to append iteration event: {}", e)))).await;
                    return;
                }

                if let Ok(agent_event) = AgentEvent::try_from(iteration_event) {
                    let _ = tx.send(Ok(agent_event)).await;
                }

                // Create LLM request
                let request = GenerateRequest {
                    messages: messages.clone(),
                    tools: Some(tool_declarations.clone()),
                    config: config.clone(),
                    system: system.clone(),
                };

                // Record LLM call started
                let llm_call_event = ThreadEvent::LlmCallStarted(LlmCallStartedData {
                    provider: "claude".to_string(), // TODO: Get from provider
                    model: "unknown".to_string(), // TODO: Get from provider metadata
                    message_count: messages.len(),
                    timestamp: Utc::now(),
                });

                let _ = store.append_event(&thread_id, llm_call_event, None, None).await;

                // Call LLM and get stream
                let llm_stream = match provider.stream_generate(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(Err(AgentError::Llm(e))).await;
                        return;
                    }
                };

                // Process LLM stream, persisting events and emitting to caller
                let mut text_content = String::new();
                let mut tool_uses = Vec::new();
                let mut content_blocks_data = Vec::new();
                let mut current_content_block_index = 0;
                let mut current_tool_use: Option<PartialToolUseAccumulator> = None;

                pin_mut!(llm_stream);

                while let Some(event_result) = llm_stream.next().await {
                    let event = match event_result {
                        Ok(e) => e,
                        Err(e) => {
                            let _ = tx.send(Err(AgentError::Llm(e))).await;
                            return;
                        }
                    };

                    // Process event and create ThreadEvents
                    match &event {
                        StreamEvent::ContentBlockStart { index, block } => {
                            current_content_block_index = *index;
                            match block {
                                ContentBlockStart::Text { text } => {
                                    text_content.push_str(text);

                                    // Emit text delta as ThreadEvent
                                    let delta_event = ThreadEvent::LlmContentDelta(LlmContentDeltaData {
                                        content_block_index: *index,
                                        delta_type: "text".to_string(),
                                        text: text.clone(),
                                        timestamp: Utc::now(),
                                    });

                                    let _ = store.append_event(&thread_id, delta_event.clone(), None, None).await;

                                    // Convert to AgentEvent and emit
                                    if let Ok(agent_event) = AgentEvent::try_from(delta_event) {
                                        let _ = tx.send(Ok(agent_event)).await;
                                    }
                                }
                                ContentBlockStart::ToolUse { id, name } => {
                                    // Record tool use started
                                    let tool_started_event = ThreadEvent::LlmToolUseStarted(LlmToolUseStartedData {
                                        tool_use_id: id.clone(),
                                        content_block_index: *index,
                                        name: name.clone(),
                                        timestamp: Utc::now(),
                                    });

                                    let _ = store.append_event(&thread_id, tool_started_event, None, None).await;

                                    current_tool_use = Some(PartialToolUseAccumulator {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: String::new(),
                                    });
                                }
                            }
                        }
                        StreamEvent::ContentDelta { index: _, delta } => {
                            match delta {
                                ContentDelta::TextDelta { text } => {
                                    text_content.push_str(text);

                                    // Emit text delta as ThreadEvent
                                    let delta_event = ThreadEvent::LlmContentDelta(LlmContentDeltaData {
                                        content_block_index: current_content_block_index,
                                        delta_type: "text".to_string(),
                                        text: text.clone(),
                                        timestamp: Utc::now(),
                                    });

                                    let _ = store.append_event(&thread_id, delta_event.clone(), None, None).await;

                                    // Convert to AgentEvent and emit
                                    if let Ok(agent_event) = AgentEvent::try_from(delta_event) {
                                        let _ = tx.send(Ok(agent_event)).await;
                                    }
                                }
                                ContentDelta::ToolUseDelta { partial } => {
                                    if let Some(tool_use) = &mut current_tool_use {
                                        tool_use.input.push_str(&partial.partial_json);

                                        // Record tool use delta
                                        let tool_delta_event = ThreadEvent::LlmToolUseDelta(LlmToolUseDeltaData {
                                            tool_use_id: tool_use.id.clone(),
                                            partial_json: partial.partial_json.clone(),
                                            timestamp: Utc::now(),
                                        });

                                        let _ = store.append_event(&thread_id, tool_delta_event, None, None).await;
                                    }
                                }
                            }
                        }
                        StreamEvent::ContentBlockEnd { index: _ } => {
                            if let Some(tool_use) = current_tool_use.take() {
                                // Parse complete tool use
                                match serde_json::from_str::<serde_json::Value>(&tool_use.input) {
                                    Ok(input) => {
                                        // Record tool use completed
                                        let tool_completed_event = ThreadEvent::LlmToolUseCompleted(LlmToolUseCompletedData {
                                            tool_use_id: tool_use.id.clone(),
                                            name: tool_use.name.clone(),
                                            input: input.clone(),
                                            timestamp: Utc::now(),
                                        });

                                        let _ = store.append_event(&thread_id, tool_completed_event, None, None).await;

                                        tool_uses.push(ContentBlock::ToolUse {
                                            id: tool_use.id.clone(),
                                            name: tool_use.name.clone(),
                                            input: input.clone(),
                                        });

                                        content_blocks_data.push(ContentBlockData::ToolUse {
                                            id: tool_use.id,
                                            name: tool_use.name,
                                            input,
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx.send(Err(AgentError::ToolInputParse(e))).await;
                                        return;
                                    }
                                }
                            }
                        }
                        StreamEvent::MessageEnd { finish_reason, usage: _ } => {
                            // Build content blocks data for the response completed event
                            if !text_content.is_empty() {
                                content_blocks_data.insert(0, ContentBlockData::Text {
                                    text: text_content.clone(),
                                });
                            }

                            // Record LLM response completed
                            let response_event = ThreadEvent::LlmResponseCompleted(LlmResponseCompletedData {
                                stop_reason: format!("{:?}", finish_reason),
                                content_blocks: content_blocks_data.clone(),
                                timestamp: Utc::now(),
                            });

                            let _ = store.append_event(&thread_id, response_event, None, None).await;
                            break;
                        }
                        _ => {}
                    }
                }

                // Check if we need to execute tools
                if tool_uses.is_empty() {
                    // Build final assistant message with text only
                    let mut assistant_content = Vec::new();
                    if !text_content.is_empty() {
                        assistant_content.push(ContentBlock::Text { text: text_content.clone() });
                    }

                    // Add to messages
                    messages.push(Message {
                        role: MessageRole::Assistant,
                        content: assistant_content,
                    });

                    // Record iteration completed
                    let iteration_completed_event = ThreadEvent::AgentIterationCompleted(AgentIterationCompletedData {
                        iteration,
                        has_tool_uses: false,
                        timestamp: Utc::now(),
                    });

                    let _ = store.append_event(&thread_id, iteration_completed_event, None, None).await;

                    // Record agent completed
                    let completed_event = ThreadEvent::AgentCompleted(AgentCompletedData {
                        total_iterations: iteration,
                        final_response: text_content,
                        timestamp: Utc::now(),
                    });

                    let _ = store.append_event(&thread_id, completed_event.clone(), None, None).await;

                    // Emit completed event
                    if let Ok(agent_event) = AgentEvent::try_from(completed_event) {
                        let _ = tx.send(Ok(agent_event)).await;
                    }

                    return;
                }

                // Build assistant message with tool uses
                let mut assistant_content = Vec::new();
                if !text_content.is_empty() {
                    assistant_content.push(ContentBlock::Text { text: text_content });
                }
                assistant_content.extend(tool_uses.clone());

                // Add to messages
                messages.push(Message {
                    role: MessageRole::Assistant,
                    content: assistant_content,
                });

                // Execute tools and add results to history
                for block in &tool_uses {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        // Record tool execution started
                        let tool_exec_started_event = ThreadEvent::ToolExecutionStarted(ToolExecutionStartedData {
                            tool_use_id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                            timestamp: Utc::now(),
                        });

                        let _ = store.append_event(&thread_id, tool_exec_started_event.clone(), None, None).await;

                        // Emit tool execution started
                        if let Ok(agent_event) = AgentEvent::try_from(tool_exec_started_event) {
                            let _ = tx.send(Ok(agent_event)).await;
                        }

                        // Execute the tool
                        match tool_executor.execute(id.clone(), name.clone(), input.clone()).await {
                            Ok(result) => {
                                // Record tool execution completed
                                let tool_exec_completed_event = ThreadEvent::ToolExecutionCompleted(ToolExecutionCompletedData {
                                    tool_use_id: id.clone(),
                                    name: name.clone(),
                                    result: result.clone(),
                                    timestamp: Utc::now(),
                                });

                                let _ = store.append_event(&thread_id, tool_exec_completed_event.clone(), None, None).await;

                                // Emit tool execution completed
                                if let Ok(agent_event) = AgentEvent::try_from(tool_exec_completed_event) {
                                    let _ = tx.send(Ok(agent_event)).await;
                                }

                                // Add tool result to messages
                                messages.push(Message::tool_result(id.clone(), result));
                            }
                            Err(error) => {
                                // Record tool execution failed
                                let tool_exec_failed_event = ThreadEvent::ToolExecutionFailed(ToolExecutionFailedData {
                                    tool_use_id: id.clone(),
                                    name: name.clone(),
                                    error: error.clone(),
                                    timestamp: Utc::now(),
                                });

                                let _ = store.append_event(&thread_id, tool_exec_failed_event.clone(), None, None).await;

                                // Emit tool execution failed
                                if let Ok(agent_event) = AgentEvent::try_from(tool_exec_failed_event) {
                                    let _ = tx.send(Ok(agent_event)).await;
                                }

                                // Add tool error to messages
                                messages.push(Message::tool_error(id.clone(), error));
                            }
                        }
                    }
                }

                // Record iteration completed
                let iteration_completed_event = ThreadEvent::AgentIterationCompleted(AgentIterationCompletedData {
                    iteration,
                    has_tool_uses: true,
                    timestamp: Utc::now(),
                });

                let _ = store.append_event(&thread_id, iteration_completed_event, None, None).await;

                // Loop continues - next iteration will call LLM again
            }
        });

        // Return the ReceiverStream which is both Send + Sync
        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    /// Get the thread store for direct access
    pub fn store(&self) -> &ThreadStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::core::error::LlmError;
    use async_trait::async_trait;

    // Mock LLM provider for testing
    struct MockProvider {
        responses: Vec<Vec<StreamEvent>>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn stream_generate(
            &self,
            _request: GenerateRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, LlmError>
        {
            let mut count = self.call_count.lock().unwrap();
            let index = *count;
            *count += 1;

            if index >= self.responses.len() {
                return Err(LlmError::StreamError("No more responses".to_string()));
            }

            let events = self.responses[index].clone();
            Ok(Box::pin(futures::stream::iter(
                events.into_iter().map(Ok),
            )))
        }
    }

    // Mock tool executor for testing
    struct MockExecutor;

    #[async_trait]
    impl ToolExecutor for MockExecutor {
        async fn execute(
            &self,
            _tool_use_id: String,
            _name: String,
            _arguments: serde_json::Value,
        ) -> Result<String, String> {
            Ok(serde_json::json!({"result": 42}).to_string())
        }
    }

    // Note: Unit tests with mock store would go here.
    // Real integration tests are in tests/agent_store_test.rs and tests/event_sourced_agent_test.rs
}
