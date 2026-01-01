//! Main agent processing loop - the heart of the agentic system.
//!
//! This module consumes submissions from the session and orchestrates:
//! - LLM calls with streaming
//! - Tool call execution
//! - Agentic loop (tool results fed back to LLM)
//! - Event emission

use async_channel::{Receiver, Sender};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use mms_common::{AgentError, AgentResult, EventId};
use mms_config::Config;
use mms_exec::{Executor, Turn, TurnId, TurnState};
use mms_protocol::{
    AgentMessageDeltaEvent, AgentMessageEvent, AgentThinkingEvent, ErrorEvent, Event,
    EventMessage, Operation, SessionId, Submission, WarningEvent,
};
use mms_providers::{
    clients::create_client, ClientConfig, CompletionRequest, CompletionResponse, FinishReason,
    FunctionCall, Message, MessageContent, MessageRole, ModelClient, StreamEvent, ToolCall as ProviderToolCall,
    ToolDefinition, Usage,
};
use mms_tools::{ToolCall as ExecToolCall, ToolOutput, ToolRegistry, ToolSpec};

use crate::state::StateManager;

/// Maximum iterations of the agentic loop to prevent infinite loops
const MAX_AGENTIC_ITERATIONS: usize = 50;

/// Processor handles the main agent loop
pub struct Processor {
    session_id: SessionId,
    config: Arc<Config>,
    state: Arc<StateManager>,
    executor: Arc<Executor>,
    submission_rx: Receiver<Submission>,
    event_tx: Sender<Event>,
    client: Box<dyn ModelClient>,
    tool_specs: Vec<ToolSpec>,
    /// Conversation history
    messages: Vec<Message>,
}

impl Processor {
    /// Create a new processor
    pub fn new(
        session_id: SessionId,
        config: Arc<Config>,
        state: Arc<StateManager>,
        executor: Arc<Executor>,
        submission_rx: Receiver<Submission>,
        event_tx: Sender<Event>,
        registry: &ToolRegistry,
    ) -> AgentResult<Self> {
        // Create the LLM client based on config
        let provider = config
            .build_provider()
            .map_err(|e| AgentError::config(format!("Failed to build provider: {}", e)))?;

        let client = create_client(provider)
            .map_err(|e| AgentError::config(format!("Failed to create client: {}", e.message)))?;

        // Get tool specs for the LLM
        let tool_specs = registry.specs();

        // Initialize with system message
        let system_prompt = build_system_prompt(&config);
        let messages = vec![Message::system(system_prompt)];

        Ok(Self {
            session_id,
            config,
            state,
            executor,
            submission_rx,
            event_tx,
            client,
            tool_specs,
            messages,
        })
    }

    /// Run the main processing loop
    pub async fn run(mut self) {
        info!(session_id = %self.session_id, "Processor started");

        while let Ok(submission) = self.submission_rx.recv().await {
            let submission_id = submission.id.to_string();
            debug!(submission_id = %submission_id, "Processing submission");

            match self.process_submission(submission).await {
                Ok(()) => {
                    debug!(submission_id = %submission_id, "Submission completed");
                }
                Err(e) => {
                    error!(submission_id = %submission_id, error = %e, "Submission failed");
                    let _ = self
                        .emit(EventMessage::Error(ErrorEvent {
                            code: "submission_error".into(),
                            message: e.to_string(),
                            recoverable: true,
                        }))
                        .await;
                }
            }
        }

        info!(session_id = %self.session_id, "Processor stopped");
    }

    /// Process a single submission
    async fn process_submission(&mut self, submission: Submission) -> AgentResult<()> {
        match submission.operation {
            Operation::UserMessage(msg) => {
                self.handle_user_message(msg.content).await
            }
            Operation::Approve(approve) => {
                // TODO: Handle approval - resume paused tool execution
                debug!(request_id = %approve.request_id, "Approval received");
                Ok(())
            }
            Operation::Reject(reject) => {
                // TODO: Handle rejection
                debug!(request_id = %reject.request_id, "Rejection received");
                Ok(())
            }
            Operation::Interrupt => {
                // TODO: Handle interrupt - stop current processing
                warn!("Interrupt received");
                Ok(())
            }
            Operation::Shutdown => {
                // Close the channel to stop the loop
                self.submission_rx.close();
                Ok(())
            }
        }
    }

    /// Handle a user message - the main agentic loop
    async fn handle_user_message(&mut self, content: String) -> AgentResult<()> {
        // Add user message to history
        self.messages.push(Message::user(&content));

        // Create a new turn
        let turn_id = TurnId::new(self.state.next_event_id());
        let mut turn = Turn::new(turn_id, self.session_id);
        turn.set_state(TurnState::Processing);

        // Run the agentic loop
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > MAX_AGENTIC_ITERATIONS {
                self.emit(EventMessage::Warning(WarningEvent {
                    code: "max_iterations".into(),
                    message: "Maximum agentic loop iterations reached".into(),
                }))
                .await?;
                break;
            }

            // Call the LLM
            let response = self.call_llm(&mut turn).await?;

            // Check if we have tool calls
            if let Some(tool_calls) = &response.message.tool_calls {
                if !tool_calls.is_empty() {
                    // Add assistant message with tool calls to history
                    self.messages.push(response.message.clone());

                    // Execute tools and get results
                    let tool_results = self.execute_tool_calls(&mut turn, tool_calls).await?;

                    // Add tool results to history
                    for result in tool_results {
                        self.messages.push(Message::tool_response(
                            &result.call_id,
                            &result.content,
                        ));
                    }

                    // Continue the loop to send results back to LLM
                    continue;
                }
            }

            // No tool calls - we're done with this turn
            // Add assistant response to history
            if let Some(text) = response.message.text() {
                if !text.is_empty() {
                    self.messages.push(response.message);
                }
            }

            break;
        }

        // Complete the turn
        self.executor.complete_turn(&turn).await?;

        Ok(())
    }

    /// Call the LLM with streaming
    async fn call_llm(
        &mut self,
        turn: &mut Turn,
    ) -> AgentResult<CompletionResponse> {
        // Build tool definitions for the LLM
        let tools: Vec<ToolDefinition> = self
            .tool_specs
            .iter()
            .map(|spec| {
                ToolDefinition::new(&spec.name)
                    .with_description(&spec.description)
                    .with_parameters(serde_json::json!({
                        "type": spec.parameters.param_type,
                        "properties": spec.parameters.properties,
                        "required": spec.parameters.required,
                    }))
            })
            .collect();

        // Build the request
        let model = self
            .config
            .model
            .clone()
            .unwrap_or_else(|| "gpt-4o".to_string());

        let client_config = ClientConfig::new(&model)
            .with_tools(tools)
            .with_stream(true);

        let request = CompletionRequest::new(self.messages.clone(), client_config);

        // Use streaming
        let mut stream = self
            .client
            .stream(request)
            .await
            .map_err(|e| AgentError::provider(format!("Stream error: {}", e.message)))?;

        // Accumulators
        let mut content = String::new();
        let mut tool_calls: HashMap<usize, ToolCallAccumulator> = HashMap::new();
        let mut finish_reason = FinishReason::Stop;
        let mut usage: Option<Usage> = None;
        let mut response_id = String::new();
        let mut response_model = String::new();

        // Process stream events
        while let Some(event_result) = stream.next().await {
            let event = event_result
                .map_err(|e| AgentError::provider(format!("Stream error: {}", e.message)))?;

            match event {
                StreamEvent::Start(start) => {
                    response_id = start.id;
                    response_model = start.model;
                }
                StreamEvent::Delta(delta) => {
                    if let Some(text) = delta.content {
                        content.push_str(&text);
                        // Emit delta event
                        self.emit(EventMessage::AgentMessageDelta(AgentMessageDeltaEvent {
                            delta: text,
                        }))
                        .await?;
                    }
                    if let Some(reasoning) = delta.reasoning {
                        // Emit thinking event
                        self.emit(EventMessage::AgentThinking(AgentThinkingEvent {
                            content: reasoning,
                        }))
                        .await?;
                    }
                }
                StreamEvent::ToolCallStart(tc_start) => {
                    tool_calls.insert(
                        tc_start.index,
                        ToolCallAccumulator {
                            id: tc_start.id,
                            name: tc_start.name,
                            arguments: String::new(),
                        },
                    );
                }
                StreamEvent::ToolCallDelta(tc_delta) => {
                    if let Some(acc) = tool_calls.get_mut(&tc_delta.index) {
                        acc.arguments.push_str(&tc_delta.arguments_delta);
                    }
                }
                StreamEvent::ToolCallEnd(_) => {
                    // Tool call complete - nothing to do here
                }
                StreamEvent::End(end) => {
                    finish_reason = end.finish_reason;
                    usage = end.usage;
                }
            }
        }

        // Update token counts
        if let Some(u) = &usage {
            turn.add_input_tokens(u.input_tokens);
            turn.add_output_tokens(u.output_tokens);
        }

        // Emit full message if there's content
        if !content.is_empty() {
            self.emit(EventMessage::AgentMessage(AgentMessageEvent {
                content: content.clone(),
            }))
            .await?;
        }

        // Convert accumulated tool calls to provider format
        let provider_tool_calls: Option<Vec<ProviderToolCall>> = if tool_calls.is_empty() {
            None
        } else {
            Some(
                tool_calls
                    .into_values()
                    .map(|acc| ProviderToolCall {
                        id: acc.id,
                        r#type: "function".into(),
                        function: FunctionCall {
                            name: acc.name,
                            arguments: acc.arguments,
                        },
                    })
                    .collect(),
            )
        };

        // Build the response message
        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Text(content),
            name: None,
            tool_calls: provider_tool_calls,
            tool_call_id: None,
        };

        Ok(CompletionResponse {
            id: response_id,
            model: response_model,
            message,
            finish_reason,
            usage,
        })
    }

    /// Execute tool calls and return results
    async fn execute_tool_calls(
        &self,
        turn: &mut Turn,
        tool_calls: &[ProviderToolCall],
    ) -> AgentResult<Vec<ToolOutput>> {
        let mut outputs = Vec::with_capacity(tool_calls.len());

        for tc in tool_calls {
            // Parse arguments
            let arguments: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));

            // Create the tool call for the executor
            let exec_call = ExecToolCall::new(&tc.id, &tc.function.name, arguments);

            // Execute via executor (handles events and approval)
            let result = self.executor.execute_tool_calls(turn, vec![exec_call]).await?;

            outputs.extend(result);
        }

        Ok(outputs)
    }

    /// Emit an event
    async fn emit(&self, message: EventMessage) -> AgentResult<()> {
        let id = EventId::new(self.state.next_event_id());
        let event = Event::new(id, message);

        self.event_tx
            .send(event)
            .await
            .map_err(|_| AgentError::ChannelClosed)
    }
}

/// Accumulator for streaming tool calls
struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
}

/// Build the system prompt
fn build_system_prompt(config: &Config) -> String {
    let mut prompt = String::from(
        r#"You are a helpful AI programming assistant. You help users with software development tasks including:
- Writing and explaining code
- Debugging and fixing issues
- Refactoring and improving code
- Understanding codebases
- Answering programming questions

You have access to tools that allow you to:
- Read and write files
- Execute shell commands
- Search the web

Always be helpful, accurate, and concise. When modifying code, explain what you're changing and why.
"#,
    );

    // Add any custom instructions from config
    if let Some(instructions) = &config.custom_instructions {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(instructions);
    }

    prompt
}

/// Spawn the processor in a background task
pub fn spawn_processor(
    session_id: SessionId,
    config: Arc<Config>,
    state: Arc<StateManager>,
    executor: Arc<Executor>,
    submission_rx: Receiver<Submission>,
    event_tx: Sender<Event>,
    registry: &ToolRegistry,
) -> AgentResult<()> {
    let processor = Processor::new(
        session_id,
        config,
        state,
        executor,
        submission_rx,
        event_tx,
        registry,
    )?;

    tokio::spawn(async move {
        processor.run().await;
    });

    Ok(())
}
