//! Simple agent loop implementation
//!
//! This module provides a simple agent that:
//! - Maintains conversation history
//! - Calls the LLM and streams all responses
//! - Automatically executes tool calls
//! - Loops until getting a text-only response
//! - Returns a stream of events throughout the entire loop

mod error;

pub use error::AgentError;

use crate::llm::core::{
    config::GenerationConfig,
    provider::LlmProvider,
    types::{
        ContentBlock, ContentBlockStart, ContentDelta, GenerateRequest, Message, MessageRole,
        StreamEvent, ToolDeclaration,
    },
};
use crate::llm::tools::executor::ToolExecutor;
use async_stream::stream;
use futures::stream::Stream;
use futures::StreamExt;
use pin_utils::pin_mut;
use std::pin::Pin;

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
    IterationStarted { iteration: usize },

    /// Agent loop completed (final response with no tool calls)
    Completed,
}

/// Helper struct for accumulating partial tool use data
struct PartialToolUseAccumulator {
    id: String,
    name: String,
    input: String,
}

/// Simple agent that manages conversation history and tool execution
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

impl Agent {
    /// Create a new agent with default settings
    pub fn new(
        provider: Box<dyn LlmProvider>,
        tool_executor: Box<dyn ToolExecutor>,
        tool_declarations: Vec<ToolDeclaration>,
        config: GenerationConfig,
        system: Option<String>,
    ) -> Self {
        Self {
            provider,
            tool_executor,
            tool_declarations,
            messages: Vec::new(),
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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AgentEvent, AgentError>> + Send + '_>>, AgentError>
    {
        // Add user message to history
        self.messages.push(Message::user(user_message));

        // Create the event stream
        let stream = self.create_agent_stream();

        Ok(Box::pin(stream))
    }

    /// Get the full conversation history
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Clear conversation history (start fresh)
    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    /// Create the agent event stream
    fn create_agent_stream(
        &mut self,
    ) -> impl Stream<Item = Result<AgentEvent, AgentError>> + '_ {
        stream! {
            let mut iteration = 0;

            loop {
                iteration += 1;

                // Check max iterations before starting
                if iteration > self.max_iterations {
                    yield Err(AgentError::MaxIterationsReached(iteration - 1));
                    return;
                }

                // Emit iteration started
                yield Ok(AgentEvent::IterationStarted { iteration });

                // Create LLM request
                let request = GenerateRequest {
                    messages: self.messages.clone(),
                    tools: Some(self.tool_declarations.clone()),
                    config: self.config.clone(),
                    system: self.system.clone(),
                };

                // Call LLM and get stream
                let llm_stream = match self.provider.stream_generate(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        yield Err(AgentError::Llm(e));
                        return;
                    }
                };

                // Process LLM stream, forwarding events and accumulating data
                let mut text_content = String::new();
                let mut tool_uses = Vec::new();
                let mut current_tool_use: Option<PartialToolUseAccumulator> = None;

                pin_mut!(llm_stream);

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
                    match &event {
                        StreamEvent::ContentBlockStart { block, .. } => {
                            match block {
                                ContentBlockStart::Text { text } => {
                                    text_content.push_str(text);
                                }
                                ContentBlockStart::ToolUse { id, name } => {
                                    current_tool_use = Some(PartialToolUseAccumulator {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input: String::new(),
                                    });
                                }
                            }
                        }
                        StreamEvent::ContentDelta { delta, .. } => {
                            match delta {
                                ContentDelta::TextDelta { text } => {
                                    text_content.push_str(text);
                                }
                                ContentDelta::ToolUseDelta { partial } => {
                                    if let Some(tool_use) = &mut current_tool_use {
                                        tool_use.input.push_str(&partial.partial_json);
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

                // Check if we need to execute tools
                if tool_uses.is_empty() {
                    // Build final assistant message with text only
                    let mut assistant_content = Vec::new();
                    if !text_content.is_empty() {
                        assistant_content.push(ContentBlock::Text { text: text_content });
                    }

                    // Add to conversation history
                    self.messages.push(Message {
                        role: MessageRole::Assistant,
                        content: assistant_content,
                    });

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

                // Execute tools and add results to history
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

                // Loop continues - next iteration will call LLM again
            }
        }
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

    #[test]
    fn test_agent_creation() {
        let provider = Box::new(MockProvider {
            responses: vec![],
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        });
        let executor = Box::new(MockExecutor);
        let config = GenerationConfig::new(1024);

        let agent = Agent::new(provider, executor, vec![], config, None);

        assert_eq!(agent.messages().len(), 0);
        assert_eq!(agent.max_iterations, 10);
    }

    #[test]
    fn test_agent_with_max_iterations() {
        let provider = Box::new(MockProvider {
            responses: vec![],
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        });
        let executor = Box::new(MockExecutor);
        let config = GenerationConfig::new(1024);

        let agent = Agent::new(provider, executor, vec![], config, None).with_max_iterations(5);

        assert_eq!(agent.max_iterations, 5);
    }

    #[test]
    fn test_clear_history() {
        let provider = Box::new(MockProvider {
            responses: vec![],
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        });
        let executor = Box::new(MockExecutor);
        let config = GenerationConfig::new(1024);

        let mut agent = Agent::new(provider, executor, vec![], config, None);
        agent.messages.push(Message::user("test"));
        assert_eq!(agent.messages().len(), 1);

        agent.clear_history();
        assert_eq!(agent.messages().len(), 0);
    }
}
