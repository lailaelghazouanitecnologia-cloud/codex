use mms_protocol::SessionId;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnId(u64);

impl TurnId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TurnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnState {
    Pending,
    Processing,
    WaitingApproval,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl TurnState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Processing | Self::WaitingApproval | Self::Executing)
    }
}

pub struct Turn {
    pub id: TurnId,
    pub session_id: SessionId,
    pub state: TurnState,
    pub input_tokens: u64,
    pub output_tokens: u64,
    started_at: Instant,
}

impl Turn {
    pub fn new(id: TurnId, session_id: SessionId) -> Self {
        Self {
            id,
            session_id,
            state: TurnState::Pending,
            input_tokens: 0,
            output_tokens: 0,
            started_at: Instant::now(),
        }
    }

    pub fn duration_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    pub fn set_state(&mut self, state: TurnState) {
        self.state = state;
    }

    pub fn add_input_tokens(&mut self, count: u64) {
        self.input_tokens = self.input_tokens.saturating_add(count);
    }

    pub fn add_output_tokens(&mut self, count: u64) {
        self.output_tokens = self.output_tokens.saturating_add(count);
    }
}
