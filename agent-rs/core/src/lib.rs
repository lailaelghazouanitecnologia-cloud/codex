#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod agent;
mod session;
mod state;

pub use agent::*;
pub use session::*;
pub use state::*;

pub use agent_common::{AgentError, AgentResult};
pub use agent_config::{Config, ConfigLoader, Feature, Features};
pub use agent_exec::{Executor, Turn, TurnId, TurnState};
pub use agent_protocol::{
    ApprovalMode, Event, EventMessage, Operation, SessionConfig, SessionId, SessionState,
    Submission,
};
pub use agent_tools::{
    ToolCall, ToolContext, ToolHandler, ToolOutput, ToolRegistry, ToolRouter, ToolSpec,
};
