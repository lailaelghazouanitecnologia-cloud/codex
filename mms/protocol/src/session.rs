use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Initializing,
    Ready,
    Processing,
    WaitingApproval,
    Executing,
    Completing,
    Error,
    Shutdown,
}

impl SessionState {
    pub fn can_transition_to(&self, target: SessionState) -> bool {
        use SessionState::*;

        match (self, target) {
            (Idle, Initializing) => true,
            (Initializing, Ready) => true,
            (Initializing, Error) => true,
            (Ready, Processing) => true,
            (Ready, Shutdown) => true,
            (Processing, WaitingApproval) => true,
            (Processing, Executing) => true,
            (Processing, Completing) => true,
            (Processing, Error) => true,
            (WaitingApproval, Executing) => true,
            (WaitingApproval, Ready) => true,
            (WaitingApproval, Error) => true,
            (Executing, Processing) => true,
            (Executing, Error) => true,
            (Completing, Ready) => true,
            (Completing, Error) => true,
            (Error, Ready) => true,
            (Error, Shutdown) => true,
            (_, Shutdown) => true,
            _ => false,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionState::Shutdown)
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            SessionState::Processing
            | SessionState::Executing
            | SessionState::WaitingApproval
        )
    }
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Idle => "idle",
            Self::Initializing => "initializing",
            Self::Ready => "ready",
            Self::Processing => "processing",
            Self::WaitingApproval => "waiting_approval",
            Self::Executing => "executing",
            Self::Completing => "completing",
            Self::Error => "error",
            Self::Shutdown => "shutdown",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub provider: String,
    pub cwd: std::path::PathBuf,
    pub approval_mode: ApprovalMode,
    pub timeout_ms: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            model: String::from("default"),
            provider: String::from("openai"),
            cwd: std::env::current_dir().unwrap_or_default(),
            approval_mode: ApprovalMode::OnDanger,
            timeout_ms: mms_common::DEFAULT_TIMEOUT_MS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalMode {
    Always,
    OnDanger,
    Never,
}

impl Default for ApprovalMode {
    fn default() -> Self {
        Self::OnDanger
    }
}
