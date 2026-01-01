//! Context management for conversation history, token tracking, and truncation.
//!
//! This module provides:
//! - Rich ResponseItem types for history
//! - Token counting and estimation
//! - Truncation policies (bytes/tokens)
//! - Auto-compaction triggers
//! - History normalization
//! - Context compaction for token management

mod compaction;
mod history;
pub mod item;
pub mod token;
mod truncation;

pub use compaction::{CompactionConfig, CompactionResult, ContextCompactor};
pub use history::ContextManager;
pub use item::{
    CompactionItem, FunctionCallItem, FunctionOutputItem, GhostSnapshotItem, MessageItem,
    MessageRole, ReasoningItem, ResponseItem, SystemItem,
};
pub use token::{approx_tokens_from_bytes, estimate_tokens, RateLimitSnapshot, TokenUsageInfo};
pub use truncation::{ModelLimits, TruncationPolicy, TruncationResult};
