//! Main agent processing loop - the heart of the agentic system.
//!
//! This module consumes submissions from the session and orchestrates:
//! - LLM calls with streaming and retry logic
//! - Tool call execution (sequential or parallel)
//! - Approval flow for sensitive operations
//! - Agentic loop (tool results fed back to LLM)
//! - Cancellation and interrupt handling
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
    AgentMessageDeltaEvent, AgentMessageEvent, AgentThinkingEvent, ApprovalMode,
    ApprovalRequiredEvent, ErrorEvent, Event, EventMessage, Operation, RiskLevel,
    SessionId, Submission, WarningEvent,
};
use mms_providers::{
    clients::create_client, ClientConfig, CompletionRequest, CompletionResponse, FinishReason,
    FunctionCall, Message, MessageContent, MessageRole, ModelClient, StreamEvent,
    ToolCall as ProviderToolCall, ToolDefinition, Usage,
};
use mms_tools::{ToolCall as ExecToolCall, ToolOutput, ToolRegistry, ToolSpec};

use crate::approval::{
    approve, new_shared_manager, reject, request_command_approval, cancel_all,
    ReviewDecision, SharedApprovalManager, DEFAULT_APPROVAL_TIMEOUT,
};
use crate::cancel::CancellationToken;
use crate::context::{
    ContextManager, FunctionCallItem, MessageItem, MessageRole as ContextMessageRole,
    ModelLimits, ResponseItem, SystemItem, TokenUsageInfo,
};
use crate::parallel::{ParallelConfig, ParallelExecutor};
use crate::retry::{RetryConfig, RetryState, RetryableError};
use crate::state::StateManager;

/// Maximum iterations of the agentic loop to prevent infinite loops
const MAX_AGENTIC_ITERATIONS: usize = 50;

/// Default maximum concurrent tool executions
const DEFAULT_PARALLEL_TOOL_CALLS: usize = 5;

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
    /// Conversation history with context management
    context: ContextManager,
    /// Approval manager for tool execution gating
    approval_manager: SharedApprovalManager,
    /// Cancellation token for the current processing
    cancel_token: CancellationToken,
    /// Parallel executor for tool calls
    parallel_executor: ParallelExecutor,
    /// Retry configuration
    retry_config: RetryConfig,
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

        // Initialize context manager with model limits
        let model_limits = Self::get_model_limits(config.model.as_deref());
        let mut context = ContextManager::with_model_limits(model_limits);

        // Add system message
        let system_prompt = build_system_prompt(&config);
        context.record(ResponseItem::System(SystemItem {
            content: system_prompt,
            is_developer: false,
        }));

        // Create approval manager
        let approval_manager = new_shared_manager();

        // Create parallel executor
        let parallel_config = ParallelConfig::new(DEFAULT_PARALLEL_TOOL_CALLS)
            .with_preserve_order(true);
        let parallel_executor = ParallelExecutor::new(parallel_config);

        // Create retry config
        let retry_config = RetryConfig::default();

        Ok(Self {
            session_id,
            config,
            state,
            executor,
            submission_rx,
            event_tx,
            client,
            tool_specs,
            context,
            approval_manager,
            cancel_token: CancellationToken::new(),
            parallel_executor,
            retry_config,
        })
    }

    /// Get model limits based on model name
    fn get_model_limits(model: Option<&str>) -> ModelLimits {
        match model {
            Some(m) if m.starts_with("gpt-4o") => ModelLimits::gpt4o(),
            Some(m) if m.starts_with("gpt-4") => ModelLimits::gpt4(),
            Some(m) if m.starts_with("claude-3-5") => ModelLimits::claude35_sonnet(),
            Some(m) if m.starts_with("claude-3") => ModelLimits::claude3(),
            _ => ModelLimits::default(),
        }
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
                // Reset cancel token for new user message
                self.cancel_token = CancellationToken::new();
                self.handle_user_message(msg.content).await
            }
            Operation::Approve(op) => {
                debug!(request_id = %op.request_id, "Approval received");
                approve(&self.approval_manager, &op.request_id).await;
                Ok(())
            }
            Operation::Reject(op) => {
                debug!(request_id = %op.request_id, "Rejection received");
                reject(&self.approval_manager, &op.request_id).await;
                Ok(())
            }
            Operation::Interrupt => {
                warn!("Interrupt received");
                // Cancel current processing
                self.cancel_token.cancel();
                // Cancel all pending approvals
                cancel_all(&self.approval_manager).await;
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
        // Add user message to context
        self.context.record(ResponseItem::Message(MessageItem {
            role: ContextMessageRole::User,
            content: content.clone(),
            name: None,
        }));

        // Create a new turn
        let turn_id = TurnId::new(self.state.next_event_id());
        let mut turn = Turn::new(turn_id, self.session_id);
        turn.set_state(TurnState::Processing);

        // Run the agentic loop
        let mut iterations = 0;

        loop {
            // Check for cancellation
            if self.cancel_token.is_cancelled() {
                debug!("Processing cancelled");
                break;
            }

            iterations += 1;
            if iterations > MAX_AGENTIC_ITERATIONS {
                self.emit(EventMessage::Warning(WarningEvent {
                    code: "max_iterations".into(),
                    message: "Maximum agentic loop iterations reached".into(),
                }))
                .await?;
                break;
            }

            // Check if we should auto-compact
            if self.context.should_compact() {
                debug!("Context auto-compaction triggered");
                // TODO: Implement context compaction
            }

            // Call the LLM with retry
            let response = match self.call_llm_with_retry(&mut turn).await {
                Ok(r) => r,
                Err(e) => {
                    if self.cancel_token.is_cancelled() {
                        debug!("LLM call cancelled");
                        break;
                    }
                    return Err(e);
                }
            };

            // Check if we have tool calls
            if let Some(tool_calls) = &response.message.tool_calls {
                if !tool_calls.is_empty() {
                    // Record assistant message with tool calls
                    for tc in tool_calls {
                        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        let needs_approval = self.needs_approval(&tc.function.name);
                        self.context.record(ResponseItem::FunctionCall(FunctionCallItem {
                            call_id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments: args,
                            requires_approval: needs_approval,
                            approval_status: None,
                        }));
                    }

                    // Execute tools and get results
                    let tool_results = self.execute_tool_calls_parallel(&mut turn, tool_calls).await?;

                    // Record tool results
                    for result in &tool_results {
                        self.context.record_output(
                            result.call_id.clone(),
                            result.content.clone(),
                            !result.success,  // is_error is inverse of success
                        );
                    }

                    // Continue the loop to send results back to LLM
                    continue;
                }
            }

            // No tool calls - we're done with this turn
            // Record assistant response
            if let Some(text) = response.message.text() {
                if !text.is_empty() {
                    self.context.record(ResponseItem::Message(MessageItem {
                        role: ContextMessageRole::Assistant,
                        content: text.to_string(),
                        name: None,
                    }));
                }
            }

            break;
        }

        // Complete the turn
        self.executor.complete_turn(&turn).await?;

        Ok(())
    }

    /// Call the LLM with retry logic
    async fn call_llm_with_retry(&mut self, turn: &mut Turn) -> AgentResult<CompletionResponse> {
        let mut retry_state = RetryState::new(self.retry_config.clone());

        loop {
            match self.call_llm(turn).await {
                Ok(response) => {
                    retry_state.record_success();
                    return Ok(response);
                }
                Err(e) => {
                    // Determine if error is retryable
                    let retryable = classify_error(&e);

                    if let Some(delay) = retry_state.record_error(retryable) {
                        warn!(
                            error = %e,
                            attempt = retry_state.attempt(),
                            "LLM call failed, retrying"
                        );

                        // Sleep with cancellation support
                        tokio::select! {
                            _ = self.cancel_token.cancelled() => {
                                return Err(AgentError::Cancelled);
                            }
                            _ = tokio::time::sleep(delay) => {}
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Call the LLM with streaming
    async fn call_llm(&mut self, turn: &mut Turn) -> AgentResult<CompletionResponse> {
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

        // Get messages from context
        let messages = self.build_messages_for_request();
        let request = CompletionRequest::new(messages, client_config);

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

        // Process stream events with cancellation support
        loop {
            tokio::select! {
                biased;

                _ = self.cancel_token.cancelled() => {
                    debug!("Stream cancelled");
                    return Err(AgentError::Cancelled);
                }

                event_opt = stream.next() => {
                    match event_opt {
                        Some(Ok(event)) => {
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
                        Some(Err(e)) => {
                            return Err(AgentError::provider(format!("Stream error: {}", e.message)));
                        }
                        None => break,
                    }
                }
            }
        }

        // Update token counts
        if let Some(u) = &usage {
            turn.add_input_tokens(u.input_tokens);
            turn.add_output_tokens(u.output_tokens);

            // Update context token tracking
            self.context.update_token_usage(&TokenUsageInfo::new(
                u.input_tokens,
                u.output_tokens,
            ));
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

    /// Build messages for LLM request from context
    fn build_messages_for_request(&self) -> Vec<Message> {
        let history = self.context.get_history_for_prompt();
        let mut messages = Vec::with_capacity(history.len());

        for item in history {
            match item {
                ResponseItem::System(s) => {
                    messages.push(Message::system(&s.content));
                }
                ResponseItem::Message(m) => {
                    match m.role {
                        ContextMessageRole::User => {
                            messages.push(Message::user(&m.content));
                        }
                        ContextMessageRole::Assistant => {
                            messages.push(Message::assistant(&m.content));
                        }
                        ContextMessageRole::System => {
                            messages.push(Message::system(&m.content));
                        }
                        ContextMessageRole::Tool => {
                            // Tool messages are handled via FunctionOutput
                        }
                    }
                }
                ResponseItem::FunctionCall(f) => {
                    // Create assistant message with tool call
                    let tc = ProviderToolCall {
                        id: f.call_id,
                        r#type: "function".into(),
                        function: FunctionCall {
                            name: f.name,
                            arguments: f.arguments.to_string(),
                        },
                    };
                    let mut msg = Message::assistant("");
                    msg.tool_calls = Some(vec![tc]);
                    messages.push(msg);
                }
                ResponseItem::FunctionOutput(o) => {
                    messages.push(Message::tool_response(&o.call_id, &o.content));
                }
                _ => {
                    // Skip other item types (Reasoning, Compaction, GhostSnapshot)
                }
            }
        }

        messages
    }

    /// Execute tool calls in parallel with approval flow
    async fn execute_tool_calls_parallel(
        &self,
        turn: &mut Turn,
        tool_calls: &[ProviderToolCall],
    ) -> AgentResult<Vec<ToolOutput>> {
        // For now, execute sequentially with approval
        // TODO: Implement true parallel execution with approval
        let mut outputs = Vec::with_capacity(tool_calls.len());

        for tc in tool_calls {
            // Check cancellation
            if self.cancel_token.is_cancelled() {
                debug!("Tool execution cancelled");
                break;
            }

            // Check if approval is required
            let needs_approval = self.needs_approval(&tc.function.name);

            if needs_approval {
                // Request approval
                let request_id = format!("{}_{}", self.session_id, tc.id);

                // Emit approval request event
                self.emit(EventMessage::ApprovalRequired(ApprovalRequiredEvent {
                    request_id: request_id.clone(),
                    tool_name: tc.function.name.clone(),
                    description: format!("Execute {} with args: {}", tc.function.name, tc.function.arguments),
                    risk_level: RiskLevel::Medium,
                }))
                .await?;

                // Wait for approval
                let decision = request_command_approval(
                    &self.approval_manager,
                    request_id.clone(),
                    tc.function.name.clone(),
                    vec![tc.function.arguments.clone()],
                    std::env::current_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    DEFAULT_APPROVAL_TIMEOUT,
                )
                .await;

                match decision {
                    ReviewDecision::Approved => {
                        debug!(tool = %tc.function.name, "Tool approved");
                    }
                    ReviewDecision::Rejected => {
                        debug!(tool = %tc.function.name, "Tool rejected");
                        outputs.push(ToolOutput::error(
                            &tc.id,
                            "Tool execution rejected by user",
                            0,
                        ));
                        continue;
                    }
                    ReviewDecision::TimedOut => {
                        debug!(tool = %tc.function.name, "Approval timed out");
                        outputs.push(ToolOutput::error(
                            &tc.id,
                            "Approval request timed out",
                            0,
                        ));
                        continue;
                    }
                    ReviewDecision::Cancelled => {
                        debug!(tool = %tc.function.name, "Approval cancelled");
                        outputs.push(ToolOutput::error(
                            &tc.id,
                            "Tool execution cancelled",
                            0,
                        ));
                        continue;
                    }
                }
            }

            // Parse arguments
            let arguments: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));

            // Create the tool call for the executor
            let exec_call = ExecToolCall::new(&tc.id, &tc.function.name, arguments);

            // Execute via executor
            let result = self.executor.execute_tool_calls(turn, vec![exec_call]).await?;

            outputs.extend(result);
        }

        Ok(outputs)
    }

    /// Check if a tool requires approval
    fn needs_approval(&self, tool_name: &str) -> bool {
        // Get approval mode from config (defaults to OnDanger)
        let approval_mode = self.config.approval_mode;

        match approval_mode {
            ApprovalMode::Never => false,
            ApprovalMode::Always => true,
            ApprovalMode::OnDanger => {
                // OnDanger mode: require approval for shell/write operations
                matches!(tool_name, "shell" | "write" | "bash" | "execute")
            }
        }
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

/// Classify an error for retry purposes
fn classify_error(error: &AgentError) -> RetryableError {
    let msg = error.to_string().to_lowercase();

    if msg.contains("timeout") {
        RetryableError::Timeout
    } else if msg.contains("rate limit") || msg.contains("429") {
        RetryableError::RateLimited(None)
    } else if msg.contains("connection") || msg.contains("network") {
        RetryableError::ConnectionFailed
    } else if msg.contains("500") || msg.contains("502") || msg.contains("503") {
        RetryableError::ServerError(500)
    } else if msg.contains("stream") && msg.contains("interrupt") {
        RetryableError::StreamInterrupted
    } else {
        RetryableError::NonRetryable
    }
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
