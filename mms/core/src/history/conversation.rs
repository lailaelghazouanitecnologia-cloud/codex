//! Conversation manager for handling multiple conversations.
//!
//! This module provides management of conversations including
//! creation, resumption, and forking of conversations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::session::ConversationId;

use super::rollout::{
    RolloutItem, RolloutLine, RolloutRecorder, SessionMeta,
    load_rollout_history, get_session_meta,
};

/// Errors that can occur in conversation management.
#[derive(Debug, Error)]
pub enum ConversationError {
    /// Rollout error.
    #[error("Rollout error: {0}")]
    Rollout(#[from] super::rollout::RolloutError),

    /// Conversation not found.
    #[error("Conversation not found: {0}")]
    NotFound(ConversationId),

    /// Invalid operation.
    #[error("Invalid operation: {0}")]
    Invalid(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for conversation operations.
pub type ConversationResult<T> = Result<T, ConversationError>;

/// Initial history for a conversation.
#[derive(Debug, Clone)]
pub enum InitialHistory {
    /// Brand new conversation.
    New,

    /// Resumed from existing rollout.
    Resumed {
        conversation_id: ConversationId,
        history: Vec<RolloutLine>,
        rollout_path: PathBuf,
    },

    /// Forked from existing conversation.
    Forked {
        source_id: ConversationId,
        history: Vec<RolloutLine>,
    },
}

/// A conversation with its associated state.
#[derive(Debug)]
pub struct Conversation {
    /// Conversation ID.
    pub id: ConversationId,

    /// Working directory.
    pub cwd: PathBuf,

    /// Session metadata.
    pub meta: SessionMeta,

    /// Rollout recorder.
    recorder: Option<Arc<RolloutRecorder>>,

    /// History items.
    history: Vec<RolloutLine>,

    /// Whether the conversation is active.
    active: bool,
}

impl Conversation {
    /// Create a new conversation.
    pub async fn new(
        base_dir: &Path,
        cwd: PathBuf,
        originator: &str,
        version: &str,
    ) -> ConversationResult<Self> {
        let id = ConversationId::new();

        let meta = SessionMeta {
            id: id.clone(),
            timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            cwd: cwd.clone(),
            originator: originator.to_string(),
            version: version.to_string(),
            instructions: None,
            model_provider: None,
            model: None,
        };

        let recorder = RolloutRecorder::new(base_dir, &id, meta.clone()).await?;

        Ok(Self {
            id,
            cwd,
            meta,
            recorder: Some(Arc::new(recorder)),
            history: Vec::new(),
            active: true,
        })
    }

    /// Resume a conversation from a rollout file.
    pub async fn resume(
        base_dir: &Path,
        rollout_path: &Path,
    ) -> ConversationResult<Self> {
        let history = load_rollout_history(rollout_path).await?;

        let meta = get_session_meta(&history)
            .cloned()
            .ok_or_else(|| ConversationError::Invalid(
                "Rollout file has no session metadata".to_string()
            ))?;

        let id = meta.id.clone();
        let cwd = meta.cwd.clone();

        // Create new recorder that appends to the same session
        let recorder = RolloutRecorder::new(base_dir, &id, meta.clone()).await?;

        Ok(Self {
            id,
            cwd,
            meta,
            recorder: Some(Arc::new(recorder)),
            history,
            active: true,
        })
    }

    /// Fork a conversation up to a specific message.
    pub async fn fork(
        base_dir: &Path,
        source: &Conversation,
        up_to_message: usize,
        originator: &str,
        version: &str,
    ) -> ConversationResult<Self> {
        let id = ConversationId::new();

        // Count user messages and collect history up to that point
        let mut user_message_count = 0;
        let mut forked_history = Vec::new();

        for line in &source.history {
            if let RolloutItem::UserMessage(_) = &line.item {
                user_message_count += 1;
                if user_message_count > up_to_message {
                    break;
                }
            }
            forked_history.push(line.clone());
        }

        let meta = SessionMeta {
            id: id.clone(),
            timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            cwd: source.cwd.clone(),
            originator: originator.to_string(),
            version: version.to_string(),
            instructions: source.meta.instructions.clone(),
            model_provider: source.meta.model_provider.clone(),
            model: source.meta.model.clone(),
        };

        let recorder = RolloutRecorder::new(base_dir, &id, meta.clone()).await?;

        // Record the forked history items
        for line in &forked_history {
            if !matches!(line.item, RolloutItem::SessionMeta(_)) {
                recorder.record_item(line.item.clone()).await?;
            }
        }

        Ok(Self {
            id,
            cwd: source.cwd.clone(),
            meta,
            recorder: Some(Arc::new(recorder)),
            history: forked_history,
            active: true,
        })
    }

    /// Record an item to the conversation.
    pub async fn record(&self, item: RolloutItem) -> ConversationResult<()> {
        if let Some(recorder) = &self.recorder {
            recorder.record_item(item).await?;
        }
        Ok(())
    }

    /// Record multiple items.
    pub async fn record_items(&self, items: Vec<RolloutItem>) -> ConversationResult<()> {
        if let Some(recorder) = &self.recorder {
            recorder.record_items(items).await?;
        }
        Ok(())
    }

    /// Get the conversation history.
    pub fn history(&self) -> &[RolloutLine] {
        &self.history
    }

    /// Get the rollout path.
    pub fn rollout_path(&self) -> Option<&Path> {
        self.recorder.as_ref().map(|r| r.path())
    }

    /// Check if the conversation is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Deactivate the conversation.
    pub async fn deactivate(&mut self) -> ConversationResult<()> {
        self.active = false;
        if let Some(recorder) = &self.recorder {
            recorder.flush().await?;
        }
        Ok(())
    }

    /// Shutdown the conversation.
    pub async fn shutdown(&mut self) -> ConversationResult<()> {
        self.active = false;
        if let Some(recorder) = &self.recorder {
            recorder.shutdown().await?;
        }
        self.recorder = None;
        Ok(())
    }

    /// Get user message count.
    pub fn user_message_count(&self) -> usize {
        self.history
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::UserMessage(_)))
            .count()
    }
}

/// Conversation manager for handling multiple conversations.
pub struct ConversationManager {
    /// Base directory for persistence.
    base_dir: PathBuf,

    /// Active conversations.
    conversations: RwLock<HashMap<ConversationId, Arc<RwLock<Conversation>>>>,

    /// Current active conversation.
    current: RwLock<Option<ConversationId>>,

    /// Originator string.
    originator: String,

    /// Version string.
    version: String,
}

impl ConversationManager {
    /// Create a new conversation manager.
    pub fn new(base_dir: PathBuf, originator: &str, version: &str) -> Self {
        Self {
            base_dir,
            conversations: RwLock::new(HashMap::new()),
            current: RwLock::new(None),
            originator: originator.to_string(),
            version: version.to_string(),
        }
    }

    /// Create a new conversation.
    pub async fn create(&self, cwd: PathBuf) -> ConversationResult<ConversationId> {
        let conversation = Conversation::new(
            &self.base_dir,
            cwd,
            &self.originator,
            &self.version,
        )
        .await?;

        let id = conversation.id.clone();

        let mut conversations = self.conversations.write().await;
        conversations.insert(id.clone(), Arc::new(RwLock::new(conversation)));

        // Set as current
        let mut current = self.current.write().await;
        *current = Some(id.clone());

        info!("Created new conversation: {}", id);

        Ok(id)
    }

    /// Resume a conversation from a rollout file.
    pub async fn resume(&self, rollout_path: &Path) -> ConversationResult<ConversationId> {
        let conversation = Conversation::resume(&self.base_dir, rollout_path).await?;

        let id = conversation.id.clone();

        let mut conversations = self.conversations.write().await;
        conversations.insert(id.clone(), Arc::new(RwLock::new(conversation)));

        // Set as current
        let mut current = self.current.write().await;
        *current = Some(id.clone());

        info!("Resumed conversation: {}", id);

        Ok(id)
    }

    /// Fork an existing conversation.
    pub async fn fork(
        &self,
        source_id: &ConversationId,
        up_to_message: usize,
    ) -> ConversationResult<ConversationId> {
        let conversations = self.conversations.read().await;

        let source_conv = conversations
            .get(source_id)
            .ok_or_else(|| ConversationError::NotFound(source_id.clone()))?;

        let source = source_conv.read().await;

        let forked = Conversation::fork(
            &self.base_dir,
            &source,
            up_to_message,
            &self.originator,
            &self.version,
        )
        .await?;

        let id = forked.id.clone();
        drop(source);
        drop(conversations);

        let mut conversations = self.conversations.write().await;
        conversations.insert(id.clone(), Arc::new(RwLock::new(forked)));

        // Set as current
        let mut current = self.current.write().await;
        *current = Some(id.clone());

        info!("Forked conversation {} from {}", id, source_id);

        Ok(id)
    }

    /// Get a conversation by ID.
    pub async fn get(&self, id: &ConversationId) -> Option<Arc<RwLock<Conversation>>> {
        let conversations = self.conversations.read().await;
        conversations.get(id).cloned()
    }

    /// Get the current conversation.
    pub async fn current(&self) -> Option<Arc<RwLock<Conversation>>> {
        let current = self.current.read().await;
        if let Some(id) = &*current {
            self.get(id).await
        } else {
            None
        }
    }

    /// Set the current conversation.
    pub async fn set_current(&self, id: &ConversationId) -> ConversationResult<()> {
        let conversations = self.conversations.read().await;
        if !conversations.contains_key(id) {
            return Err(ConversationError::NotFound(id.clone()));
        }

        let mut current = self.current.write().await;
        *current = Some(id.clone());

        Ok(())
    }

    /// Remove a conversation.
    pub async fn remove(&self, id: &ConversationId) -> ConversationResult<()> {
        let mut conversations = self.conversations.write().await;

        if let Some(conv) = conversations.remove(id) {
            let mut conv = conv.write().await;
            conv.shutdown().await?;
        }

        // Clear current if it was this conversation
        let mut current = self.current.write().await;
        if current.as_ref() == Some(id) {
            *current = None;
        }

        Ok(())
    }

    /// List all conversation IDs.
    pub async fn list(&self) -> Vec<ConversationId> {
        let conversations = self.conversations.read().await;
        conversations.keys().cloned().collect()
    }

    /// Shutdown all conversations.
    pub async fn shutdown(&self) {
        let mut conversations = self.conversations.write().await;

        for (_, conv) in conversations.drain() {
            let mut conv = conv.write().await;
            if let Err(e) = conv.shutdown().await {
                warn!("Error shutting down conversation: {}", e);
            }
        }

        let mut current = self.current.write().await;
        *current = None;

        debug!("Conversation manager shutdown complete");
    }
}

/// Type alias for shared conversation manager.
pub type SharedConversationManager = Arc<ConversationManager>;

/// Create a new shared conversation manager.
pub fn new_conversation_manager(
    base_dir: PathBuf,
    originator: &str,
    version: &str,
) -> SharedConversationManager {
    Arc::new(ConversationManager::new(base_dir, originator, version))
}
