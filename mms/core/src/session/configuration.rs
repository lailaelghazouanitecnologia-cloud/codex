//! Session configuration and settings management.
//!
//! This module provides the configuration structures that define how a session
//! operates, including model settings, approval policies, and working directory.

use std::path::PathBuf;
use std::sync::Arc;

use mms_config::Config;
use mms_protocol::{ApprovalMode, SandboxPolicy, SessionSource};
use serde::{Deserialize, Serialize};

/// Reasoning effort level for models that support it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    #[default]
    Medium,
    High,
}

/// How to handle reasoning output in responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningSummary {
    /// Include full reasoning in response
    Full,
    /// Include summarized reasoning
    #[default]
    Summary,
    /// Hide reasoning from response
    Hidden,
}

/// Constrained value that can be locked to prevent changes.
#[derive(Debug, Clone)]
pub struct Constrained<T> {
    value: T,
    locked: bool,
}

impl<T: Clone> Constrained<T> {
    /// Create a new constrained value that can be modified.
    pub fn new(value: T) -> Self {
        Self {
            value,
            locked: false,
        }
    }

    /// Create a new constrained value that is locked.
    pub fn locked(value: T) -> Self {
        Self {
            value,
            locked: true,
        }
    }

    /// Get the current value.
    pub fn value(&self) -> T {
        self.value.clone()
    }

    /// Get a reference to the current value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Check if the value is locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Try to set a new value. Returns error if locked.
    pub fn set(&mut self, value: T) -> Result<(), ConstraintError> {
        if self.locked {
            return Err(ConstraintError::Locked);
        }
        self.value = value;
        Ok(())
    }
}

/// Error when trying to modify a constrained value.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConstraintError {
    #[error("value is locked and cannot be modified")]
    Locked,
}

/// Result type for constraint operations.
pub type ConstraintResult<T> = Result<T, ConstraintError>;

/// Session configuration that defines how a session operates.
///
/// This is the immutable configuration that is set when a session is created.
/// Some values can be updated during the session via `SessionSettingsUpdate`.
#[derive(Debug, Clone)]
pub struct SessionConfiguration {
    /// Provider identifier ("openai", "anthropic", "groq", ...).
    pub provider: String,

    /// Model identifier. If not specified, provider's default is used.
    pub model: String,

    /// Reasoning effort for models that support it.
    pub model_reasoning_effort: Option<ReasoningEffort>,

    /// How to handle reasoning in responses.
    pub model_reasoning_summary: ReasoningSummary,

    /// Developer instructions that supplement the base instructions.
    pub developer_instructions: Option<String>,

    /// User instructions that are appended to the system prompt.
    pub user_instructions: Option<String>,

    /// Base instructions override for the system prompt.
    pub base_instructions: Option<String>,

    /// Custom prompt for context compaction.
    pub compact_prompt: Option<String>,

    /// When to escalate for approval for execution.
    pub approval_policy: Constrained<ApprovalMode>,

    /// How to sandbox commands executed in the system.
    pub sandbox_policy: Constrained<SandboxPolicy>,

    /// Working directory that is treated as the root of the session.
    /// All relative paths are resolved against this directory.
    pub cwd: PathBuf,

    /// Source of the session (cli, vscode, exec, mcp, web, ...).
    pub session_source: SessionSource,

    /// Reference to the original config (for per-turn config building).
    pub(crate) original_config: Arc<Config>,
}

impl SessionConfiguration {
    /// Create a new session configuration from a config.
    pub fn from_config(config: Arc<Config>, session_source: SessionSource) -> Self {
        Self {
            provider: config.provider_id.clone(),
            model: config.model.clone().unwrap_or_else(|| "gpt-4o".to_string()),
            model_reasoning_effort: None,
            model_reasoning_summary: ReasoningSummary::default(),
            developer_instructions: None,
            user_instructions: config.custom_instructions.clone(),
            base_instructions: None,
            compact_prompt: None,
            approval_policy: Constrained::new(config.approval_mode),
            sandbox_policy: Constrained::new(SandboxPolicy::default()),
            cwd: config.cwd.clone(),
            session_source,
            original_config: config,
        }
    }

    /// Apply updates to the configuration.
    pub fn apply(&self, updates: &SessionSettingsUpdate) -> ConstraintResult<Self> {
        let mut next = self.clone();

        if let Some(model) = &updates.model {
            next.model = model.clone();
        }
        if let Some(effort) = updates.reasoning_effort {
            next.model_reasoning_effort = effort;
        }
        if let Some(summary) = updates.reasoning_summary {
            next.model_reasoning_summary = summary;
        }
        if let Some(approval) = updates.approval_policy {
            next.approval_policy.set(approval)?;
        }
        if let Some(sandbox) = &updates.sandbox_policy {
            next.sandbox_policy.set(sandbox.clone())?;
        }
        if let Some(cwd) = &updates.cwd {
            next.cwd = cwd.clone();
        }

        Ok(next)
    }

    /// Resolve a path relative to the session's working directory.
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
}

/// Updates that can be applied to a session configuration.
#[derive(Debug, Clone, Default)]
pub struct SessionSettingsUpdate {
    /// New working directory.
    pub cwd: Option<PathBuf>,
    /// New approval policy.
    pub approval_policy: Option<ApprovalMode>,
    /// New sandbox policy.
    pub sandbox_policy: Option<SandboxPolicy>,
    /// New model.
    pub model: Option<String>,
    /// New reasoning effort.
    pub reasoning_effort: Option<Option<ReasoningEffort>>,
    /// New reasoning summary mode.
    pub reasoning_summary: Option<ReasoningSummary>,
    /// JSON schema for final output validation.
    pub final_output_json_schema: Option<Option<serde_json::Value>>,
}

impl SessionSettingsUpdate {
    /// Create a new empty update.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the working directory.
    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Set the approval policy.
    pub fn with_approval_policy(mut self, policy: ApprovalMode) -> Self {
        self.approval_policy = Some(policy);
        self
    }
}
