//! Context manager for conversation history.
//!
//! Handles:
//! - Recording and managing response items
//! - Token tracking and estimation
//! - History truncation and normalization
//! - Auto-compaction triggers

use std::collections::HashSet;

use super::compaction::{CompactionConfig, CompactionResult, ContextCompactor};
use super::item::{FunctionCallItem, FunctionOutputItem, MessageRole, ResponseItem};
use super::token::{RateLimitSnapshot, TokenUsageInfo};
use super::truncation::{ModelLimits, TruncationPolicy};

/// Manages conversation history with token tracking
#[derive(Debug, Default)]
pub struct ContextManager {
    /// The conversation history items
    items: Vec<ResponseItem>,
    /// Cumulative token usage
    token_info: TokenUsageInfo,
    /// Current rate limits
    rate_limits: Option<RateLimitSnapshot>,
    /// Model-specific limits
    model_limits: ModelLimits,
    /// Truncation policy for tool outputs
    output_policy: TruncationPolicy,
}

impl ContextManager {
    /// Create a new context manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific model limits
    pub fn with_model_limits(model_limits: ModelLimits) -> Self {
        let output_policy = model_limits.tool_output_policy();
        Self {
            items: Vec::new(),
            token_info: TokenUsageInfo::default(),
            rate_limits: None,
            model_limits,
            output_policy,
        }
    }

    /// Record a single item to history
    pub fn record(&mut self, item: ResponseItem) {
        self.items.push(item);
    }

    /// Record multiple items to history
    pub fn record_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = ResponseItem>,
    {
        self.items.extend(items);
    }

    /// Record a function output with automatic truncation
    pub fn record_output(&mut self, call_id: String, content: String, is_error: bool) {
        let result = self.output_policy.truncate(&content);

        let output = if result.truncated {
            FunctionOutputItem {
                call_id,
                content: result.content,
                is_error,
                original_bytes: Some(result.original_bytes),
                truncated: true,
            }
        } else {
            FunctionOutputItem {
                call_id,
                content,
                is_error,
                original_bytes: None,
                truncated: false,
            }
        };

        self.items.push(ResponseItem::FunctionOutput(output));
    }

    /// Get all history items
    pub fn items(&self) -> &[ResponseItem] {
        &self.items
    }

    /// Get history items prepared for model input
    pub fn get_history_for_prompt(&self) -> Vec<ResponseItem> {
        let mut history = self.items.clone();

        // Remove ghost snapshots (they're just for tracking)
        history.retain(|item| !matches!(item, ResponseItem::GhostSnapshot(_)));

        // Normalize: remove orphaned function outputs
        self.remove_orphaned_outputs(&mut history);

        history
    }

    /// Remove function outputs that don't have corresponding function calls
    fn remove_orphaned_outputs(&self, history: &mut Vec<ResponseItem>) {
        // Collect all call IDs from function calls
        let call_ids: HashSet<_> = history
            .iter()
            .filter_map(|item| match item {
                ResponseItem::FunctionCall(f) => Some(f.call_id.clone()),
                _ => None,
            })
            .collect();

        // Remove outputs without matching calls
        history.retain(|item| {
            match item {
                ResponseItem::FunctionOutput(o) => call_ids.contains(&o.call_id),
                _ => true,
            }
        });
    }

    /// Estimate total token count of current history
    pub fn estimate_total_tokens(&self) -> u64 {
        self.items.iter().map(|item| item.estimate_tokens()).sum()
    }

    /// Check if we should trigger auto-compaction
    pub fn should_compact(&self) -> bool {
        let current_tokens = self.estimate_total_tokens();
        self.model_limits.should_compact(current_tokens)
    }

    /// Update token usage from a turn
    pub fn update_token_usage(&mut self, usage: &TokenUsageInfo) {
        self.token_info.merge(usage);
    }

    /// Get current token usage
    pub fn token_info(&self) -> &TokenUsageInfo {
        &self.token_info
    }

    /// Update rate limits
    pub fn update_rate_limits(&mut self, limits: RateLimitSnapshot) {
        self.rate_limits = Some(limits);
    }

    /// Get current rate limits
    pub fn rate_limits(&self) -> Option<&RateLimitSnapshot> {
        self.rate_limits.as_ref()
    }

    /// Get token info and rate limits together
    pub fn token_info_and_rate_limits(&self) -> (&TokenUsageInfo, Option<&RateLimitSnapshot>) {
        (&self.token_info, self.rate_limits.as_ref())
    }

    /// Remove the first (oldest) item from history
    pub fn remove_first(&mut self) -> Option<ResponseItem> {
        if self.items.is_empty() {
            return None;
        }

        let removed = self.items.remove(0);

        // Also remove corresponding items (e.g., function output for function call)
        if let Some(call_id) = removed.call_id() {
            self.items.retain(|item| {
                item.call_id().map(|id| id != call_id).unwrap_or(true)
            });
        }

        Some(removed)
    }

    /// Remove items until we're under the token threshold
    pub fn truncate_to_fit(&mut self, target_tokens: u64) {
        while self.estimate_total_tokens() > target_tokens && !self.items.is_empty() {
            // Don't remove the system message if it's first
            if matches!(self.items.first(), Some(ResponseItem::System(_))) && self.items.len() == 1 {
                break;
            }

            // Skip system message at the start
            let start_idx = if matches!(self.items.first(), Some(ResponseItem::System(_))) {
                1
            } else {
                0
            };

            if start_idx >= self.items.len() {
                break;
            }

            // Remove from the beginning (oldest items first)
            let removed = self.items.remove(start_idx);

            // Remove corresponding items
            if let Some(call_id) = removed.call_id() {
                let call_id = call_id.to_string();
                self.items.retain(|item| {
                    item.call_id().map(|id| id != call_id).unwrap_or(true)
                });
            }
        }
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get the number of items in history
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the last assistant message
    pub fn last_assistant_message(&self) -> Option<&str> {
        self.items.iter().rev().find_map(|item| {
            match item {
                ResponseItem::Message(m) if m.role == MessageRole::Assistant => Some(m.content.as_str()),
                _ => None,
            }
        })
    }

    /// Count items by role
    pub fn count_by_role(&self, role: MessageRole) -> usize {
        self.items
            .iter()
            .filter(|item| item.role() == Some(role))
            .count()
    }

    /// Get all function calls that are pending (no output yet)
    pub fn pending_function_calls(&self) -> Vec<&FunctionCallItem> {
        let output_call_ids: HashSet<_> = self
            .items
            .iter()
            .filter_map(|item| match item {
                ResponseItem::FunctionOutput(o) => Some(o.call_id.as_str()),
                _ => None,
            })
            .collect();

        self.items
            .iter()
            .filter_map(|item| match item {
                ResponseItem::FunctionCall(f) if !output_call_ids.contains(f.call_id.as_str()) => {
                    Some(f)
                }
                _ => None,
            })
            .collect()
    }

    /// Set model limits
    pub fn set_model_limits(&mut self, limits: ModelLimits) {
        self.output_policy = limits.tool_output_policy();
        self.model_limits = limits;
    }

    /// Set truncation policy for outputs
    pub fn set_output_policy(&mut self, policy: TruncationPolicy) {
        self.output_policy = policy;
    }

    /// Perform context compaction to reduce token usage
    ///
    /// This summarizes older items while preserving recent context.
    pub fn compact(&mut self) -> CompactionResult {
        let config = CompactionConfig::with_target(self.model_limits.compaction_target());
        self.compact_with_config(config)
    }

    /// Perform context compaction with custom configuration
    pub fn compact_with_config(&mut self, config: CompactionConfig) -> CompactionResult {
        let compactor = ContextCompactor::new(config);
        let history = std::mem::take(&mut self.items);
        let (new_history, result) = compactor.compact(history);
        self.items = new_history;
        result
    }

    /// Auto-compact if needed, returning the result if compaction was performed
    pub fn auto_compact(&mut self) -> Option<CompactionResult> {
        if self.should_compact() {
            Some(self.compact())
        } else {
            None
        }
    }
}
