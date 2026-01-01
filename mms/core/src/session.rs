use mms_common::{AgentError, AgentResult, EventId, SubmissionId, CHANNEL_CAPACITY};
use mms_config::Config;
use mms_exec::Executor;
use mms_protocol::{
    Event, EventMessage, Operation, SessionId, SessionStartedEvent, SessionState,
    SessionStateChangedEvent, Submission,
};
use mms_tools::{ToolRegistry, ToolRouter};
use async_channel::{Receiver, Sender};
use std::sync::Arc;

use crate::state::StateManager;

pub struct Session {
    id: SessionId,
    config: Arc<Config>,
    state: Arc<StateManager>,
    #[allow(dead_code)]
    executor: Arc<Executor>,
    submission_tx: Sender<Submission>,
    submission_rx: Receiver<Submission>,
    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
}

impl Session {
    pub fn new(config: Config, registry: ToolRegistry) -> AgentResult<Self> {
        let id = SessionId::new();
        let config = Arc::new(config);
        let state = Arc::new(StateManager::new());

        let (submission_tx, submission_rx) = async_channel::bounded(CHANNEL_CAPACITY);
        let (event_tx, event_rx) = async_channel::unbounded();
        let (internal_event_tx, internal_event_rx) = async_channel::unbounded::<EventMessage>();

        let router = Arc::new(ToolRouter::new(Arc::new(registry)));
        let executor = Arc::new(Executor::new(config.clone(), router, internal_event_tx));

        let session = Self {
            id,
            config,
            state,
            executor,
            submission_tx,
            submission_rx,
            event_tx: event_tx.clone(),
            event_rx,
        };

        session.spawn_event_forwarder(internal_event_rx, event_tx);

        Ok(session)
    }

    fn spawn_event_forwarder(
        &self,
        internal_rx: Receiver<EventMessage>,
        event_tx: Sender<Event>,
    ) {
        let state = self.state.clone();

        tokio::spawn(async move {
            while let Ok(message) = internal_rx.recv().await {
                let id = EventId::new(state.next_event_id());
                let event = Event::new(id, message);

                if event_tx.send(event).await.is_err() {
                    break;
                }
            }
        });
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn state(&self) -> SessionState {
        self.state.current()
    }

    pub async fn start(&self) -> AgentResult<()> {
        let previous = self.state.transition(SessionState::Initializing)?;

        self.emit(EventMessage::SessionStateChanged(SessionStateChangedEvent {
            previous,
            current: SessionState::Initializing,
        }))
        .await?;

        self.state.transition(SessionState::Ready)?;

        self.emit(EventMessage::SessionStarted(SessionStartedEvent {
            session_id: self.id.to_string(),
            model: self.config.model.clone().unwrap_or_default(),
            provider: self.config.provider_id.clone(),
        }))
        .await?;

        self.emit(EventMessage::SessionStateChanged(SessionStateChangedEvent {
            previous: SessionState::Initializing,
            current: SessionState::Ready,
        }))
        .await?;

        Ok(())
    }

    pub async fn submit(&self, operation: Operation) -> AgentResult<SubmissionId> {
        let id = SubmissionId::new(self.state.next_event_id());
        let submission = Submission::new(id, operation);

        self.submission_tx
            .send(submission)
            .await
            .map_err(|_| AgentError::ChannelClosed)?;

        Ok(id)
    }

    pub async fn next_event(&self) -> AgentResult<Event> {
        self.event_rx
            .recv()
            .await
            .map_err(|_| AgentError::ChannelClosed)
    }

    pub fn submission_receiver(&self) -> Receiver<Submission> {
        self.submission_rx.clone()
    }

    pub fn event_sender(&self) -> Sender<Event> {
        self.event_tx.clone()
    }

    async fn emit(&self, message: EventMessage) -> AgentResult<()> {
        let id = EventId::new(self.state.next_event_id());
        let event = Event::new(id, message);

        self.event_tx
            .send(event)
            .await
            .map_err(|_| AgentError::ChannelClosed)
    }

    pub async fn shutdown(&self) -> AgentResult<()> {
        self.state.transition(SessionState::Shutdown)?;
        self.emit(EventMessage::ShutdownComplete).await?;
        Ok(())
    }
}
