#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod agent;
pub mod approval;
pub mod cancel;
pub mod context;
pub mod parallel;
mod processor;
pub mod retry;
mod session;
mod state;

pub use agent::*;
pub use approval::{ApprovalManager, ReviewDecision, SharedApprovalManager};
pub use cancel::{CancellationToken, ChildCancellationToken, CancelledError, CancelOnDrop};
pub use context::{ContextManager, ResponseItem, TokenUsageInfo, TruncationPolicy, ModelLimits};
pub use parallel::{ParallelConfig, ParallelExecutor, ToolExecutionResult, DependentToolCall, ToolScheduler, execute_parallel, batch_by_dependency};
pub use retry::{RetryConfig, RetryState, RetryableError, with_retry};
pub use processor::{spawn_processor, Processor};
pub use session::*;
pub use state::*;

pub use mms_common::{AgentError, AgentResult};
pub use mms_config::{Config, ConfigLoader, Feature, Features};
pub use mms_exec::{Executor, Turn, TurnId, TurnState};
pub use mms_protocol::{
    ApprovalMode, Event, EventMessage, Operation, SessionConfig, SessionId, SessionState,
    Submission,
};
pub use mms_tools::{
    ToolCall, ToolContext, ToolHandler, ToolOutput, ToolRegistry, ToolRouter, ToolSpec,
};
