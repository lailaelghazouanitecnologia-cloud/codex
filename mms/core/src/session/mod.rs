//! Session management for the agent.
//!
//! This module provides the core session infrastructure.

mod configuration;
mod services;
mod turn;

pub use configuration::*;
pub use services::*;
pub use turn::*;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use tokio::sync::Mutex;
use tracing::{debug, info};

use mms_common::{AgentError, AgentResult, SubmissionId};
use mms_config::Config;
use mms_exec::Executor;
use mms_protocol::{
    Event, EventMessage, Operation, SessionId, SessionState, Submission,
};
use mms_tools::{ToolRegistry, ToolRouter};

use crate::processor::spawn_processor;
use crate::state::StateManager;

/// Channel capacity for internal communication.
const CHANNEL_CAPACITY: usize = 64;

/// Unique identifier for a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ConversationId(u64);

impl ConversationId {
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self((nanos & 0xFFFFFFFFFFFFFFFF) as u64)
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Initial history for a session.
#[derive(Debug, Clone)]
pub enum InitialHistory {
    New,
    Forked(ConversationId),
    Resumed(ResumedHistory),
}

/// Resumed session history.
#[derive(Debug, Clone)]
pub struct ResumedHistory {
    pub conversation_id: ConversationId,
    pub rollout_path: std::path::PathBuf,
}

impl InitialHistory {
    pub fn conversation_id(&self) -> Option<ConversationId> {
        match self {
            InitialHistory::Resumed(h) => Some(h.conversation_id),
            _ => None,
        }
    }
}

/// Session struct that wraps the processor.
pub struct Session {
    session_id: SessionId,
    conversation_id: ConversationId,
    config: Arc<Config>,
    registry: Arc<ToolRegistry>,
    tx_submission: Option<Sender<Submission>>,
    rx_event: Option<Receiver<Event>>,
    state: Mutex<SessionState>,
    next_submission_id: AtomicU64,
    started: Mutex<bool>,
}

impl Session {
    /// Create a new session.
    pub fn new(config: Config, registry: ToolRegistry) -> AgentResult<Self> {
        let session_id = SessionId::new();
        let conversation_id = ConversationId::new();

        debug!(
            session_id = %session_id,
            conversation_id = %conversation_id,
            "Creating new session"
        );

        Ok(Self {
            session_id,
            conversation_id,
            config: Arc::new(config),
            registry: Arc::new(registry),
            tx_submission: None,
            rx_event: None,
            state: Mutex::new(SessionState::Idle),
            next_submission_id: AtomicU64::new(0),
            started: Mutex::new(false),
        })
    }

    pub fn id(&self) -> SessionId {
        self.session_id
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn state(&self) -> SessionState {
        self.state
            .try_lock()
            .map(|guard| *guard)
            .unwrap_or(SessionState::Processing)
    }

    pub async fn start(&mut self) -> AgentResult<()> {
        let mut started = self.started.lock().await;
        if *started {
            return Err(AgentError::invalid_state("started", "idle"));
        }

        // Create channels
        let (tx_submission, rx_submission) = async_channel::bounded(CHANNEL_CAPACITY);
        let (tx_event, rx_event) = async_channel::unbounded::<Event>();

        // Create an event message channel for the executor
        let (event_msg_tx, _event_msg_rx) = async_channel::unbounded::<EventMessage>();

        // Update state
        {
            let mut state = self.state.lock().await;
            *state = SessionState::Initializing;
        }

        // Create executor components
        let router = Arc::new(ToolRouter::new(Arc::clone(&self.registry)));
        let executor = Arc::new(Executor::new(
            Arc::clone(&self.config),
            router,
            event_msg_tx,
        ));

        // Create state manager
        let state_manager = Arc::new(StateManager::new());

        // Spawn processor
        spawn_processor(
            self.session_id,
            Arc::clone(&self.config),
            state_manager,
            executor,
            rx_submission,
            tx_event.clone(),
            &self.registry,
        )?;

        // Store channels
        self.tx_submission = Some(tx_submission);
        self.rx_event = Some(rx_event);

        // Update state
        {
            let mut state = self.state.lock().await;
            *state = SessionState::Ready;
        }

        *started = true;

        info!(session_id = %self.session_id, "Session started");
        Ok(())
    }

    pub async fn submit(&self, operation: Operation) -> AgentResult<SubmissionId> {
        let tx = self
            .tx_submission
            .as_ref()
            .ok_or_else(|| AgentError::invalid_state("none", "started"))?;

        let id = self.next_submission_id.fetch_add(1, Ordering::SeqCst);
        let submission_id = SubmissionId::new(id);

        let submission = Submission {
            id: submission_id.clone(),
            operation,
        };

        tx.send(submission)
            .await
            .map_err(|_| AgentError::ChannelClosed)?;

        Ok(submission_id)
    }

    pub async fn next_event(&self) -> AgentResult<Event> {
        let rx = self
            .rx_event
            .as_ref()
            .ok_or_else(|| AgentError::invalid_state("none", "started"))?;

        rx.recv().await.map_err(|_| AgentError::ChannelClosed)
    }

    pub async fn shutdown(&self) -> AgentResult<()> {
        if let Some(tx) = &self.tx_submission {
            let id = self.next_submission_id.fetch_add(1, Ordering::SeqCst);
            let submission = Submission {
                id: SubmissionId::new(id),
                operation: Operation::Shutdown,
            };
            let _ = tx.send(submission).await;
        }

        {
            let mut state = self.state.lock().await;
            *state = SessionState::Shutdown;
        }

        info!(session_id = %self.session_id, "Session shutdown");
        Ok(())
    }
}
