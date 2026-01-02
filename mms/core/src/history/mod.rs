//! Message history and conversation persistence.
//!
//! This module provides:
//! - Rollout recording for JSONL-based session persistence
//! - Conversation management for handling multiple conversations
//! - Session resumption and forking
//!
//! # Example
//!
//! ```ignore
//! use mms_core::history::{ConversationManager, RolloutItem};
//! use std::path::PathBuf;
//!
//! // Create conversation manager
//! let manager = ConversationManager::new(
//!     PathBuf::from("~/.mms"),
//!     "cli",
//!     "0.1.0",
//! );
//!
//! // Create new conversation
//! let cwd = std::env::current_dir()?;
//! let id = manager.create(cwd).await?;
//!
//! // Record items
//! if let Some(conv) = manager.current().await {
//!     let conv = conv.read().await;
//!     conv.record(RolloutItem::UserMessage(UserMessageItem {
//!         content: "Hello!".to_string(),
//!         images: None,
//!     })).await?;
//! }
//! ```

pub mod conversation;
pub mod rollout;

pub use conversation::{
    Conversation, ConversationError, ConversationManager, ConversationResult,
    InitialHistory, SharedConversationManager, new_conversation_manager,
};
pub use rollout::{
    AssistantMessageItem, CompactedItem, ReasoningItem, RolloutError, RolloutItem,
    RolloutLine, RolloutRecorder, RolloutResult, SessionMeta, SharedRolloutRecorder,
    ToolCallItem, ToolResultItem, TurnContextItem, UserMessageItem,
    get_session_meta, list_rollout_files, load_rollout_history,
};
