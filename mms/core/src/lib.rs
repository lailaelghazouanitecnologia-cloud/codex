#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod agent;
pub mod approval;
pub mod cancel;
pub mod context;
pub mod parallel;
mod processor;
pub mod retry;
pub mod session;
mod state;

pub use agent::*;
pub use approval::{
    ApprovalManager, ApprovalStore, ApprovalType, ReviewDecision, SharedApprovalManager,
    approve, cancel_all, new_shared_manager, reject, request_command_approval,
    request_patch_approval, DEFAULT_APPROVAL_TIMEOUT,
};
pub use cancel::{CancellationToken, ChildCancellationToken, CancelledError, CancelOnDrop};
pub use context::{ContextManager, ResponseItem, TokenUsageInfo, TruncationPolicy, ModelLimits};
pub use parallel::{ParallelConfig, ParallelExecutor, ToolExecutionResult, DependentToolCall, ToolScheduler, execute_parallel, batch_by_dependency};
pub use retry::{RetryConfig, RetryState, RetryableError, with_retry};
pub use processor::{spawn_processor, Processor};
pub use session::{
    // Configuration
    SessionConfiguration, SessionSettingsUpdate, Constrained, ConstraintError,
    ReasoningEffort, ReasoningSummary,
    // Services
    SessionServices, ShellInfo, ShellType, McpSessionManager, UserNotifier, NotificationConfig,
    // Turn management
    TurnContext, ActiveTurn, RunningTask, TaskKind, ToolsConfig,
    PendingInput, ReviewDecision as TurnReviewDecision,
    // Session
    Session, ConversationId, InitialHistory, ResumedHistory,
};
pub use state::*;

pub use mms_common::{AgentError, AgentResult};
pub use mms_config::{Config, ConfigLoader, Feature, Features};
pub use mms_exec::{Executor, Turn, TurnId, TurnState as ExecTurnState};
pub use mms_execpolicy::ExecPolicyManager;
pub use mms_protocol::{
    ApprovalMode, Event, EventMessage, Operation, SessionConfig, SessionId,
    SessionState as ProtocolSessionState, SandboxPolicy, SessionSource, Submission,
};
pub use mms_tools::{
    ToolCall, ToolContext, ToolHandler, ToolOutput, ToolRegistry, ToolRouter, ToolSpec,
};
