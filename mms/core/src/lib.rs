#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod agent;
mod session;
mod state;

pub use agent::*;
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
