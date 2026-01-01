//! Tests for context compaction functionality.

use mms_core::context::{
    CompactionConfig, ContextCompactor, ContextManager, ModelLimits, ResponseItem,
};

#[test]
fn test_compaction_config_default() {
    let config = CompactionConfig::default();
    assert_eq!(config.target_tokens, 50_000);
    assert_eq!(config.preserve_recent, 10);
    assert!(config.create_snapshots);
}

#[test]
fn test_compaction_config_builder() {
    let config = CompactionConfig::with_target(30_000)
        .preserve_recent(5)
        .with_snapshots(false);

    assert_eq!(config.target_tokens, 30_000);
    assert_eq!(config.preserve_recent, 5);
    assert!(!config.create_snapshots);
}

#[test]
fn test_compactor_no_compaction_needed() {
    let config = CompactionConfig::with_target(100_000);
    let compactor = ContextCompactor::new(config);

    let history = vec![
        ResponseItem::user_message("Hello"),
        ResponseItem::assistant_message("Hi there!"),
    ];

    let (new_history, result) = compactor.compact(history);

    assert_eq!(result.items_removed, 0);
    assert!(!result.summary_item.is_some());
    assert_eq!(new_history.len(), 2);
}

#[test]
fn test_compactor_compacts_old_items() {
    // Create a config with a very low target to force compaction
    let config = CompactionConfig::with_target(10) // Very low target
        .preserve_recent(2)
        .with_snapshots(false);

    let compactor = ContextCompactor::new(config);

    // Create history with many items
    let mut history = vec![ResponseItem::system("System prompt")];
    for i in 0..10 {
        history.push(ResponseItem::user_message(format!("User message {}", i)));
        history.push(ResponseItem::assistant_message(format!(
            "Assistant response {}",
            i
        )));
    }

    let (new_history, result) = compactor.compact(history);

    // Should have removed some items
    assert!(result.items_removed > 0);
    // Should have created a summary
    assert!(result.summary_item.is_some());
    // Preserved items should be less than original
    assert!(new_history.len() < 21);
    // System message should still be first
    assert!(matches!(new_history.first(), Some(ResponseItem::System(_))));
}

#[test]
fn test_compactor_preserves_system_message() {
    let config = CompactionConfig::with_target(10)
        .preserve_recent(1)
        .with_snapshots(false);

    let compactor = ContextCompactor::new(config);

    let history = vec![
        ResponseItem::system("Important system prompt"),
        ResponseItem::user_message("Old message 1"),
        ResponseItem::user_message("Old message 2"),
        ResponseItem::user_message("Recent message"),
    ];

    let (new_history, _result) = compactor.compact(history);

    // System message should always be preserved
    assert!(matches!(new_history.first(), Some(ResponseItem::System(_))));
}

#[test]
fn test_compactor_creates_ghost_snapshots() {
    let config = CompactionConfig::with_target(10)
        .preserve_recent(1)
        .with_snapshots(true);

    let compactor = ContextCompactor::new(config);

    let history = vec![
        ResponseItem::user_message("Old message to snapshot"),
        ResponseItem::assistant_message("Response to snapshot"),
        ResponseItem::user_message("Recent message"),
    ];

    let (_new_history, result) = compactor.compact(history);

    // Should have created ghost snapshots
    assert!(!result.ghost_snapshots.is_empty());
}

#[test]
fn test_compaction_summary_content() {
    let config = CompactionConfig::with_target(10)
        .preserve_recent(1)
        .with_snapshots(false);

    let compactor = ContextCompactor::new(config);

    let history = vec![
        ResponseItem::user_message("User question"),
        ResponseItem::assistant_message("Assistant answer"),
        ResponseItem::function_call("call_1", "test_tool", serde_json::json!({})),
        ResponseItem::function_output("call_1", "Tool output", false),
        ResponseItem::user_message("Recent message"),
    ];

    let (_new_history, result) = compactor.compact(history);

    if let Some(ResponseItem::Compaction(c)) = &result.summary_item {
        // Summary should mention the compacted items
        assert!(c.summary.contains("Compacted"));
        assert!(c.turns_compacted > 0);
    } else {
        panic!("Expected compaction item");
    }
}

#[test]
fn test_context_manager_compact() {
    // Create a custom compaction config with low target
    let config = CompactionConfig::with_target(10)
        .preserve_recent(2)
        .with_snapshots(false);

    let limits = ModelLimits::gpt4();
    let mut manager = ContextManager::with_model_limits(limits);

    // Add system message
    manager.record(ResponseItem::system("System prompt"));

    // Add many messages
    for i in 0..20 {
        manager.record(ResponseItem::user_message(format!(
            "A very long user message number {} with lots of content.",
            i
        )));
        manager.record(ResponseItem::assistant_message(format!(
            "A very long assistant response number {} with verbose content.",
            i
        )));
    }

    let initial_len = manager.len();

    // Compact with custom config
    let result = manager.compact_with_config(config);

    // Should have removed items
    assert!(result.items_removed > 0, "Should have removed items");
    // With preserve_recent=2 and no snapshots, length should decrease significantly
    assert!(manager.len() < initial_len, "Length should decrease");
}

#[test]
fn test_context_manager_auto_compact() {
    let mut limits = ModelLimits::gpt4();
    // Set a very low threshold to trigger compaction
    limits.auto_compact_threshold = 100;

    let mut manager = ContextManager::with_model_limits(limits);

    // Add just enough content to trigger compaction
    manager.record(ResponseItem::system("System prompt"));
    manager.record(ResponseItem::user_message(
        "A message that will push us over the threshold",
    ));

    // This should trigger auto-compact if over threshold
    let _result = manager.auto_compact();

    // Whether it compacted or not depends on threshold
    // Just ensure it doesn't panic
}

#[test]
fn test_compaction_empty_history() {
    let config = CompactionConfig::default();
    let compactor = ContextCompactor::new(config);

    let (new_history, result) = compactor.compact(vec![]);

    assert!(new_history.is_empty());
    assert_eq!(result.items_removed, 0);
    assert!(result.summary_item.is_none());
}

#[test]
fn test_compaction_result_fields() {
    let config = CompactionConfig::with_target(10)
        .preserve_recent(1)
        .with_snapshots(false);

    let compactor = ContextCompactor::new(config);

    let history = vec![
        ResponseItem::user_message("First message"),
        ResponseItem::user_message("Second message"),
        ResponseItem::user_message("Third message (preserved)"),
    ];

    let tokens_before: u64 = history.iter().map(|i| i.estimate_tokens()).sum();
    let (_new_history, result) = compactor.compact(history);

    // tokens_before should be recorded
    assert_eq!(result.tokens_before, tokens_before);
    // tokens_after should be recorded (may be larger due to summary overhead, or smaller)
    assert!(result.tokens_after > 0);
    // items_removed should be positive since we compacted
    assert!(result.items_removed > 0);
}

#[test]
fn test_model_limits_compaction_target() {
    let limits = ModelLimits::gpt4();

    // compaction_target should be half of auto_compact_threshold
    assert_eq!(limits.compaction_target(), limits.auto_compact_threshold / 2);
}
