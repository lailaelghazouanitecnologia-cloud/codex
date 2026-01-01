use serde::{Deserialize, Serialize};

use crate::session::SessionState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventMessage {
    SessionStarted(SessionStartedEvent),
    SessionStateChanged(SessionStateChangedEvent),
    AgentThinking(AgentThinkingEvent),
    AgentMessage(AgentMessageEvent),
    AgentMessageDelta(AgentMessageDeltaEvent),
    ToolCallStarted(ToolCallStartedEvent),
    ToolCallCompleted(ToolCallCompletedEvent),
    ApprovalRequired(ApprovalRequiredEvent),
    Error(ErrorEvent),
    Warning(WarningEvent),
    TurnCompleted(TurnCompletedEvent),
    ShutdownComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartedEvent {
    pub session_id: String,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateChangedEvent {
    pub previous: SessionState,
    pub current: SessionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThinkingEvent {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageEvent {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageDeltaEvent {
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStartedEvent {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallCompletedEvent {
    pub call_id: String,
    pub tool_name: String,
    pub result: ToolResult,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolResult {
    Success { output: String },
    Error { message: String },
    Timeout { duration_ms: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequiredEvent {
    pub request_id: String,
    pub tool_name: String,
    pub description: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningEvent {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnCompletedEvent {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_ms: u64,
}
