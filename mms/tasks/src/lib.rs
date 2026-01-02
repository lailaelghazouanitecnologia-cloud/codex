//! Task system for common agent operations.
//!
//! This crate provides structured task implementations for:
//! - **Undo**: Revert file changes from previous turns
//! - **Review**: Code review assistance (planned)
//! - **Compact**: Context summarization (planned)
//!
//! # Example
//!
//! ```ignore
//! use mms_tasks::{UndoTask, UndoConfig, UndoResult};
//! use mms_git::turn_diff::TurnDiff;
//!
//! // Create an undo task from a turn diff
//! let diff: TurnDiff = /* from turn diff tracker */;
//! let task = UndoTask::new(diff);
//!
//! // Execute the undo
//! let result = task.execute().await?;
//! println!("Undone {} changes: {}", result.changes_undone, result.summary);
//!
//! // Or use with configuration
//! let config = UndoConfig::new()
//!     .with_files(vec!["specific_file.rs".into()])
//!     .with_backups();
//! let task = UndoTask::with_config(diff, config);
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

pub mod undo;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Re-exports
pub use undo::{
    undo_from_tracker, preview_undo,
    UndoConfig, UndoResult, UndoTask, RevertType, RevertedFile, FailedRevert,
};

/// Errors that can occur during task execution.
#[derive(Debug, Error)]
pub enum TaskError {
    /// IO error during task execution.
    #[error("IO error: {0}")]
    Io(String),

    /// Task was cancelled.
    #[error("Task cancelled")]
    Cancelled,

    /// Invalid input provided to task.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Task failed with a specific reason.
    #[error("Task failed: {0}")]
    Failed(String),

    /// Task timed out.
    #[error("Task timed out after {0}ms")]
    Timeout(u64),

    /// Generic error from another source.
    #[error("{0}")]
    Other(String),
}

impl TaskError {
    /// Create an IO error.
    pub fn io(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }

    /// Create an invalid input error.
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a not found error.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a failed error.
    pub fn failed(msg: impl Into<String>) -> Self {
        Self::Failed(msg.into())
    }

    /// Create a timeout error.
    pub fn timeout(ms: u64) -> Self {
        Self::Timeout(ms)
    }
}

/// Result type for task operations.
pub type TaskResult<T> = Result<T, TaskError>;

/// Context provided to tasks during execution.
#[derive(Debug, Clone)]
pub struct TaskContext {
    /// Current working directory.
    pub cwd: std::path::PathBuf,

    /// Session ID for the current session.
    pub session_id: String,

    /// Turn ID for the current turn (if any).
    pub turn_id: Option<String>,
}

impl TaskContext {
    /// Create a new task context.
    pub fn new(cwd: impl Into<std::path::PathBuf>, session_id: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            session_id: session_id.into(),
            turn_id: None,
        }
    }

    /// Set the turn ID.
    pub fn with_turn(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }
}

/// Output from a task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    /// Whether the task succeeded.
    pub success: bool,

    /// Human-readable message describing the result.
    pub message: String,

    /// Structured data from the task (task-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl TaskOutput {
    /// Create a success output.
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    /// Create a failure output.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }

    /// Add data to the output.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// A task that can be executed by the agent.
#[async_trait]
pub trait Task: Send + Sync {
    /// Get the task name.
    fn name(&self) -> &str;

    /// Get a description of what the task does.
    fn description(&self) -> &str;

    /// Execute the task.
    async fn execute(&self, ctx: &TaskContext) -> TaskResult<TaskOutput>;

    /// Check if the task can be cancelled.
    fn cancellable(&self) -> bool {
        true
    }
}

/// Registry for available tasks.
#[derive(Default)]
pub struct TaskRegistry {
    tasks: std::collections::HashMap<String, Box<dyn Task>>,
}

impl TaskRegistry {
    /// Create a new task registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a task.
    pub fn register<T: Task + 'static>(&mut self, task: T) {
        self.tasks.insert(task.name().to_string(), Box::new(task));
    }

    /// Get a task by name.
    pub fn get(&self, name: &str) -> Option<&dyn Task> {
        self.tasks.get(name).map(|t| t.as_ref())
    }

    /// List all registered task names.
    pub fn list(&self) -> Vec<&str> {
        self.tasks.keys().map(|s| s.as_str()).collect()
    }

    /// Execute a task by name.
    pub async fn execute(&self, name: &str, ctx: &TaskContext) -> TaskResult<TaskOutput> {
        let task = self.get(name).ok_or_else(|| {
            TaskError::not_found(format!("Task not found: {}", name))
        })?;

        task.execute(ctx).await
    }
}

/// Prelude for commonly used types.
pub mod prelude {
    pub use crate::{
        Task, TaskContext, TaskError, TaskOutput, TaskRegistry, TaskResult,
        UndoConfig, UndoResult, UndoTask,
    };
}
