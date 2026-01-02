//! Turn context and state management.
//!
//! A "turn" represents a single user message and the agent's response,
//! including any tool calls and their results.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;
use tokio::sync::{Mutex, Notify, oneshot};
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use mms_protocol::{ApprovalMode, SandboxPolicy};
use mms_providers::{ModelClient, ClientConfig};
use mms_tools::ToolSpec;

use super::configuration::{ReasoningEffort, ReasoningSummary, SessionConfiguration};
use crate::context::{TruncationPolicy, ModelLimits};

/// Context for a single turn of the conversation.
///
/// This contains everything needed to execute a turn, including the model client,
/// policies, and configuration.
#[derive(Debug)]
pub struct TurnContext {
    /// Submission ID for this turn.
    pub sub_id: String,

    /// Path to the current working directory.
    pub cwd: PathBuf,

    /// Developer instructions.
    pub developer_instructions: Option<String>,

    /// Base instructions.
    pub base_instructions: Option<String>,

    /// Compact prompt for context compaction.
    pub compact_prompt: Option<String>,

    /// User instructions.
    pub user_instructions: Option<String>,

    /// Approval policy for this turn.
    pub approval_policy: ApprovalMode,

    /// Sandbox policy for this turn.
    pub sandbox_policy: SandboxPolicy,

    /// Tool configuration.
    pub tools_config: ToolsConfig,

    /// JSON schema for final output validation.
    pub final_output_json_schema: Option<serde_json::Value>,

    /// Truncation policy for tool output.
    pub truncation_policy: TruncationPolicy,

    /// Model client for this turn.
    pub(crate) client_config: ClientConfig,

    /// Model limits.
    pub(crate) model_limits: ModelLimits,

    /// Reasoning effort for this turn.
    pub reasoning_effort: Option<ReasoningEffort>,

    /// Reasoning summary mode.
    pub reasoning_summary: ReasoningSummary,
}

impl TurnContext {
    /// Create a new turn context from session configuration.
    pub fn from_session_config(
        session_config: &SessionConfiguration,
        sub_id: String,
        _tools: Vec<ToolSpec>,
    ) -> Self {
        let client_config = ClientConfig::new(&session_config.model);

        let model_limits = ModelLimits::for_model(&session_config.model);

        Self {
            sub_id,
            cwd: session_config.cwd.clone(),
            developer_instructions: session_config.developer_instructions.clone(),
            base_instructions: session_config.base_instructions.clone(),
            compact_prompt: session_config.compact_prompt.clone(),
            user_instructions: session_config.user_instructions.clone(),
            approval_policy: session_config.approval_policy.value(),
            sandbox_policy: session_config.sandbox_policy.value(),
            tools_config: ToolsConfig::default(),
            final_output_json_schema: None,
            truncation_policy: TruncationPolicy::default(),
            client_config,
            model_limits,
            reasoning_effort: session_config.model_reasoning_effort,
            reasoning_summary: session_config.model_reasoning_summary,
        }
    }

    /// Resolve a path relative to the turn's working directory.
    pub fn resolve_path(&self, path: Option<&str>) -> PathBuf {
        match path {
            Some(p) => {
                let path = PathBuf::from(p);
                if path.is_absolute() {
                    path
                } else {
                    self.cwd.join(path)
                }
            }
            None => self.cwd.clone(),
        }
    }

    /// Get the compact prompt, using default if not set.
    pub fn compact_prompt(&self) -> &str {
        self.compact_prompt.as_deref().unwrap_or(DEFAULT_COMPACT_PROMPT)
    }
}

/// Default prompt used for context compaction.
pub const DEFAULT_COMPACT_PROMPT: &str = r#"Summarize the conversation so far, focusing on:
1. The user's original request and intent
2. Key decisions made and their rationale
3. Important code changes or file modifications
4. Current state and what remains to be done

Be concise but preserve critical technical details."#;

/// Tool configuration for a turn.
#[derive(Debug, Clone, Default)]
pub struct ToolsConfig {
    /// Whether to enable dangerous tools.
    pub allow_dangerous: bool,
    /// Specific tools to enable.
    pub enabled_tools: Option<Vec<String>>,
    /// Specific tools to disable.
    pub disabled_tools: Option<Vec<String>>,
    /// Maximum parallel tool calls.
    pub max_parallel: usize,
}

impl ToolsConfig {
    /// Create a new tools config.
    pub fn new() -> Self {
        Self {
            allow_dangerous: false,
            enabled_tools: None,
            disabled_tools: None,
            max_parallel: 5,
        }
    }

    /// Check if a tool is enabled.
    pub fn is_enabled(&self, tool_name: &str) -> bool {
        // If disabled list contains tool, it's disabled
        if let Some(disabled) = &self.disabled_tools {
            if disabled.iter().any(|t| t == tool_name) {
                return false;
            }
        }

        // If enabled list exists, tool must be in it
        if let Some(enabled) = &self.enabled_tools {
            return enabled.iter().any(|t| t == tool_name);
        }

        // Default: enabled
        true
    }
}

/// Kind of task running in a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    /// Regular user request processing.
    Regular,
    /// Code review task.
    Review,
    /// Context compaction task.
    Compact,
    /// Background task.
    Background,
}

/// A running task within an active turn.
pub struct RunningTask {
    /// Notification for when the task completes.
    pub done: Arc<Notify>,
    /// Kind of task.
    pub kind: TaskKind,
    /// Cancellation token for the task.
    pub cancellation_token: CancellationToken,
    /// Handle that aborts the task when dropped.
    pub handle: Arc<AbortOnDropHandle<()>>,
    /// Turn context for this task.
    pub turn_context: Arc<TurnContext>,
}

/// Metadata about the currently running turn.
pub struct ActiveTurn {
    /// Running tasks indexed by submission ID.
    pub tasks: IndexMap<String, RunningTask>,
    /// Mutable state for the turn.
    pub turn_state: Arc<Mutex<TurnState>>,
}

impl Default for ActiveTurn {
    fn default() -> Self {
        Self {
            tasks: IndexMap::new(),
            turn_state: Arc::new(Mutex::new(TurnState::default())),
        }
    }
}

impl ActiveTurn {
    /// Create a new active turn.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a task to the active turn.
    pub fn add_task(&mut self, task: RunningTask) {
        let sub_id = task.turn_context.sub_id.clone();
        self.tasks.insert(sub_id, task);
    }

    /// Remove a task from the active turn.
    /// Returns true if the active turn is now empty.
    pub fn remove_task(&mut self, sub_id: &str) -> bool {
        self.tasks.swap_remove(sub_id);
        self.tasks.is_empty()
    }

    /// Drain all tasks from the active turn.
    pub fn drain_tasks(&mut self) -> Vec<RunningTask> {
        self.tasks.drain(..).map(|(_, task)| task).collect()
    }

    /// Cancel all running tasks.
    pub fn cancel_all(&self) {
        for (_, task) in &self.tasks {
            task.cancellation_token.cancel();
        }
    }

    /// Clear pending approvals and input.
    pub async fn clear_pending(&self) {
        let mut ts = self.turn_state.lock().await;
        ts.clear_pending();
    }
}

/// Review decision for approval requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    /// User approved the action.
    Approved,
    /// User rejected the action.
    Rejected,
    /// Request timed out waiting for approval.
    TimedOut,
    /// Request was cancelled.
    Cancelled,
}

/// Mutable state for a single turn.
#[derive(Default)]
pub struct TurnState {
    /// Pending approval requests.
    pending_approvals: HashMap<String, oneshot::Sender<ReviewDecision>>,
    /// Pending input items to be processed.
    pending_input: Vec<PendingInput>,
}

/// A pending input item.
#[derive(Debug, Clone)]
pub struct PendingInput {
    /// Input type identifier.
    pub input_type: String,
    /// Input content.
    pub content: serde_json::Value,
}

impl TurnState {
    /// Insert a pending approval request.
    pub fn insert_pending_approval(
        &mut self,
        key: String,
        tx: oneshot::Sender<ReviewDecision>,
    ) -> Option<oneshot::Sender<ReviewDecision>> {
        self.pending_approvals.insert(key, tx)
    }

    /// Remove a pending approval request.
    pub fn remove_pending_approval(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<ReviewDecision>> {
        self.pending_approvals.remove(key)
    }

    /// Clear all pending state.
    pub fn clear_pending(&mut self) {
        // Drop all pending approval senders (will cause receivers to get errors)
        self.pending_approvals.clear();
        self.pending_input.clear();
    }

    /// Add a pending input item.
    pub fn push_pending_input(&mut self, input: PendingInput) {
        self.pending_input.push(input);
    }

    /// Take all pending input items.
    pub fn take_pending_input(&mut self) -> Vec<PendingInput> {
        std::mem::take(&mut self.pending_input)
    }

    /// Check if there are pending approvals.
    pub fn has_pending_approvals(&self) -> bool {
        !self.pending_approvals.is_empty()
    }

    /// Get the number of pending approvals.
    pub fn pending_approval_count(&self) -> usize {
        self.pending_approvals.len()
    }
}
