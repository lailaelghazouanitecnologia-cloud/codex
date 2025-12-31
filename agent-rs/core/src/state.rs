use agent_common::{AgentError, AgentResult};
use agent_protocol::SessionState;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct StateManager {
    current: std::sync::RwLock<SessionState>,
    turn_counter: AtomicU64,
    event_counter: AtomicU64,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            current: std::sync::RwLock::new(SessionState::Idle),
            turn_counter: AtomicU64::new(0),
            event_counter: AtomicU64::new(0),
        }
    }

    pub fn current(&self) -> SessionState {
        *self.current.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn transition(&self, target: SessionState) -> AgentResult<SessionState> {
        let mut guard = self.current.write().unwrap_or_else(|e| e.into_inner());
        let current = *guard;

        if !current.can_transition_to(target) {
            return Err(AgentError::invalid_state(
                current.to_string(),
                target.to_string(),
            ));
        }

        *guard = target;
        Ok(current)
    }

    pub fn next_turn_id(&self) -> u64 {
        self.turn_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_event_id(&self) -> u64 {
        self.event_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn is_active(&self) -> bool {
        self.current().is_active()
    }

    pub fn is_terminal(&self) -> bool {
        self.current().is_terminal()
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}
