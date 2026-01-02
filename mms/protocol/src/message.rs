use serde::{Deserialize, Serialize};

use crate::session::{ApprovalMode, SessionId, SessionState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventMessage {
    /// Session has been configured and is ready.
    SessionConfigured(SessionConfiguredEvent),
    /// Session has started (legacy).
    SessionStarted(SessionStartedEvent),
    /// Session state has changed.
    SessionStateChanged(SessionStateChangedEvent),
    /// Agent is thinking/reasoning.
    AgentThinking(AgentThinkingEvent),
    /// Agent has produced a complete message.
    AgentMessage(AgentMessageEvent),
    /// Agent message content delta (streaming).
    AgentMessageDelta(AgentMessageDeltaEvent),
    /// Agent reasoning section break.
    AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent),
    /// Reasoning content delta.
    ReasoningContentDelta(ReasoningContentDeltaEvent),
    /// Tool call has started.
    ToolCallStarted(ToolCallStartedEvent),
    /// Tool call has completed.
    ToolCallCompleted(ToolCallCompletedEvent),
    /// Approval is required for an action.
    ApprovalRequired(ApprovalRequiredEvent),
    /// Token count update.
    TokenCount(TokenCountEvent),
    /// Rate limit information.
    RateLimit(RateLimitEvent),
    /// Error occurred.
    Error(ErrorEvent),
    /// Warning message.
    Warning(WarningEvent),
    /// Turn has completed.
    TurnCompleted(TurnCompletedEvent),
    /// Deprecation notice.
    DeprecationNotice(DeprecationNoticeEvent),
    /// Background event from tools.
    BackgroundEvent(BackgroundEventEvent),
    /// Session shutdown complete.
    ShutdownComplete,
}

/// Session configured event - sent when session is ready.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfiguredEvent {
    pub session_id: SessionId,
    pub model: String,
    pub provider: String,
    pub approval_policy: ApprovalMode,
    pub cwd: String,
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

/// Agent reasoning section break event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReasoningSectionBreakEvent {
    pub section_type: String,
}

/// Reasoning content delta event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningContentDeltaEvent {
    pub delta: String,
    pub is_raw: bool,
}

/// Token count event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCountEvent {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub context_window: Option<u64>,
}

/// Rate limit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEvent {
    pub requests_limit: Option<i64>,
    pub requests_remaining: Option<i64>,
    pub tokens_limit: Option<i64>,
    pub tokens_remaining: Option<i64>,
    pub reset_at: Option<String>,
}

/// Deprecation notice event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationNoticeEvent {
    pub summary: String,
    pub details: Option<String>,
}

/// Background event from tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundEventEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}
