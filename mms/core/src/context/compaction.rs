//! Context compaction for managing conversation history size.
//!
//! When context exceeds threshold, older items are summarized/compacted
//! to reduce token usage while preserving essential information.

use super::item::{CompactionItem, GhostSnapshotItem, MessageRole, ResponseItem};

/// Configuration for context compaction
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Target token count after compaction
    pub target_tokens: u64,
    /// Minimum items to preserve (most recent)
    pub preserve_recent: usize,
    /// Whether to create ghost snapshots of removed items
    pub create_snapshots: bool,
    /// Maximum summary length in characters
    pub max_summary_len: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            target_tokens: 50_000,
            preserve_recent: 10,
            create_snapshots: true,
            max_summary_len: 500,
        }
    }
}

impl CompactionConfig {
    /// Create with specific target tokens
    pub fn with_target(target_tokens: u64) -> Self {
        Self {
            target_tokens,
            ..Default::default()
        }
    }

    /// Set number of recent items to preserve
    pub fn preserve_recent(mut self, count: usize) -> Self {
        self.preserve_recent = count;
        self
    }

    /// Enable/disable ghost snapshots
    pub fn with_snapshots(mut self, enabled: bool) -> Self {
        self.create_snapshots = enabled;
        self
    }
}

/// Result of a compaction operation
#[derive(Debug)]
pub struct CompactionResult {
    /// Number of items removed
    pub items_removed: usize,
    /// Number of items preserved
    pub items_preserved: usize,
    /// Estimated tokens before compaction
    pub tokens_before: u64,
    /// Estimated tokens after compaction
    pub tokens_after: u64,
    /// The compaction summary item (if created)
    pub summary_item: Option<ResponseItem>,
    /// Ghost snapshots of removed items (if enabled)
    pub ghost_snapshots: Vec<ResponseItem>,
}

/// Performs context compaction on conversation history
pub struct ContextCompactor {
    config: CompactionConfig,
}

impl ContextCompactor {
    /// Create a new compactor with config
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(CompactionConfig::default())
    }

    /// Compact the given history, returning the result and new history
    pub fn compact(&self, history: Vec<ResponseItem>) -> (Vec<ResponseItem>, CompactionResult) {
        let tokens_before: u64 = history.iter().map(|i| i.estimate_tokens()).sum();

        // If already under target, no compaction needed
        if tokens_before <= self.config.target_tokens {
            let result = CompactionResult {
                items_removed: 0,
                items_preserved: history.len(),
                tokens_before,
                tokens_after: tokens_before,
                summary_item: None,
                ghost_snapshots: vec![],
            };
            return (history, result);
        }

        // Separate system message (always preserve)
        let (system_items, mut other_items): (Vec<_>, Vec<_>) = history
            .into_iter()
            .partition(|item| matches!(item, ResponseItem::System(_)));

        // Calculate how many items to remove
        let preserve_count = self.config.preserve_recent.min(other_items.len());
        let split_point = other_items.len().saturating_sub(preserve_count);

        // Split into items to compact and items to preserve
        let preserved_items: Vec<ResponseItem> = other_items.split_off(split_point);
        let items_to_compact = other_items;

        let items_removed = items_to_compact.len();

        // Create ghost snapshots if enabled
        let ghost_snapshots: Vec<ResponseItem> = if self.config.create_snapshots {
            items_to_compact
                .iter()
                .filter_map(|item| self.create_ghost_snapshot(item))
                .collect()
        } else {
            vec![]
        };

        // Create compaction summary
        let summary_item = self.create_summary(&items_to_compact);

        // Build new history: system + compaction summary + preserved items
        let mut new_history = system_items;

        if let Some(ref summary) = summary_item {
            new_history.push(summary.clone());
        }

        // Add ghost snapshots before preserved items
        new_history.extend(ghost_snapshots.clone());
        new_history.extend(preserved_items);

        let tokens_after: u64 = new_history.iter().map(|i| i.estimate_tokens()).sum();

        let result = CompactionResult {
            items_removed,
            items_preserved: new_history.len(),
            tokens_before,
            tokens_after,
            summary_item,
            ghost_snapshots,
        };

        (new_history, result)
    }

    /// Create a ghost snapshot of an item (lightweight reference)
    /// Note: GhostSnapshotItem is designed for file snapshots, so we reuse it
    /// with path as item type and content as a summary
    fn create_ghost_snapshot(&self, item: &ResponseItem) -> Option<ResponseItem> {
        let (item_type, summary) = match item {
            ResponseItem::Message(m) => {
                let role = format!("{:?}", m.role);
                let preview = truncate_str(&m.content, 50);
                (role, preview)
            }
            ResponseItem::FunctionCall(f) => {
                ("FunctionCall".to_string(), format!("{}(...)", f.name))
            }
            ResponseItem::FunctionOutput(o) => {
                let status = if o.is_error { "error" } else { "success" };
                let preview = truncate_str(&o.content, 30);
                ("FunctionOutput".to_string(), format!("[{}] {}", status, preview))
            }
            ResponseItem::Reasoning(r) => {
                let preview = truncate_str(&r.content, 50);
                ("Reasoning".to_string(), preview)
            }
            // Don't snapshot system, compaction, or existing snapshots
            _ => return None,
        };

        // Reuse GhostSnapshotItem with path as item type
        Some(ResponseItem::GhostSnapshot(GhostSnapshotItem {
            path: item_type,
            content: summary,
            is_new: false,
        }))
    }

    /// Create a summary of compacted items
    fn create_summary(&self, items: &[ResponseItem]) -> Option<ResponseItem> {
        if items.is_empty() {
            return None;
        }

        let mut summary_parts = Vec::new();

        // Count by type
        let mut user_count = 0;
        let mut assistant_count = 0;
        let mut tool_calls = Vec::new();

        for item in items {
            match item {
                ResponseItem::Message(m) => match m.role {
                    MessageRole::User => user_count += 1,
                    MessageRole::Assistant => assistant_count += 1,
                    _ => {}
                },
                ResponseItem::FunctionCall(f) => {
                    tool_calls.push(f.name.clone());
                }
                _ => {}
            }
        }

        // Build summary text
        if user_count > 0 || assistant_count > 0 {
            summary_parts.push(format!(
                "{} user message(s), {} assistant message(s)",
                user_count, assistant_count
            ));
        }

        if !tool_calls.is_empty() {
            let unique_tools: Vec<_> = tool_calls
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            summary_parts.push(format!(
                "{} tool call(s): {}",
                tool_calls.len(),
                unique_tools.iter().take(5).cloned().cloned().collect::<Vec<_>>().join(", ")
            ));
        }

        let summary_text = format!(
            "[Compacted {} items: {}]",
            items.len(),
            summary_parts.join("; ")
        );

        // Truncate if too long
        let summary_text = truncate_str(&summary_text, self.config.max_summary_len);

        Some(ResponseItem::Compaction(CompactionItem {
            summary: summary_text,
            turns_compacted: items.len(),
            original_tokens: items.iter().map(|i| i.estimate_tokens()).sum(),
            encrypted_content: None,
        }))
    }
}

/// Truncate a string to max length, adding ellipsis if needed
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
