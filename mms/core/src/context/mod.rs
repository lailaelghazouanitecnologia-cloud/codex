//! Context management for conversation history, token tracking, and truncation.
//!
//! This module provides:
//! - Rich ResponseItem types for history
//! - Token counting and estimation
//! - Truncation policies (bytes/tokens)
//! - Auto-compaction triggers
//! - History normalization

mod history;
pub mod item;
pub mod token;
mod truncation;

pub use history::ContextManager;
pub use item::{ResponseItem, FunctionCallItem, FunctionOutputItem, ReasoningItem, SystemItem, MessageItem, MessageRole};
pub use token::{TokenUsageInfo, RateLimitSnapshot, estimate_tokens, approx_tokens_from_bytes};
pub use truncation::{TruncationPolicy, TruncationResult, ModelLimits};
