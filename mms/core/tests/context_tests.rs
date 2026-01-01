//! Tests for the context management module

use mms_core::context::{
    ContextManager, ResponseItem, TokenUsageInfo, TruncationPolicy, ModelLimits,
    token::{estimate_tokens, RateLimitSnapshot},
};

#[test]
fn test_estimate_tokens() {
    // "hello world" = 11 bytes = ~3 tokens
    assert_eq!(estimate_tokens("hello world"), 3);

    // Empty string = 0 tokens
    assert_eq!(estimate_tokens(""), 0);

    // 100 bytes = 25 tokens
    let text = "x".repeat(100);
    assert_eq!(estimate_tokens(&text), 25);
}

#[test]
fn test_token_usage_merge() {
    let mut a = TokenUsageInfo::new(100, 50);
    let b = TokenUsageInfo::new(200, 100);
    a.merge(&b);

    assert_eq!(a.input_tokens, Some(300));
    assert_eq!(a.output_tokens, Some(150));
    assert_eq!(a.total(), 450);
}

#[test]
fn test_rate_limit_near_limit() {
    let snapshot = RateLimitSnapshot {
        requests_remaining: Some(5),
        requests_limit: Some(100),
        tokens_remaining: Some(1000),
        tokens_limit: Some(10000),
        reset_seconds: Some(60.0),
    };
    assert!(snapshot.is_near_limit());

    let snapshot = RateLimitSnapshot {
        requests_remaining: Some(50),
        requests_limit: Some(100),
        tokens_remaining: Some(5000),
        tokens_limit: Some(10000),
        reset_seconds: None,
    };
    assert!(!snapshot.is_near_limit());
}

#[test]
fn test_truncation_policy_bytes() {
    let policy = TruncationPolicy::Bytes(10);

    let result = policy.truncate("hello");
    assert!(!result.truncated);
    assert_eq!(result.content, "hello");

    let result = policy.truncate("hello world, this is a long string");
    assert!(result.truncated);
    assert!(result.content.len() <= 10);
}

#[test]
fn test_truncation_policy_tokens() {
    let policy = TruncationPolicy::Tokens(3); // ~12 bytes

    let result = policy.truncate("hello");
    assert!(!result.truncated);

    let result = policy.truncate("this is a much longer string that exceeds the limit");
    assert!(result.truncated);
}

#[test]
fn test_model_limits_compact() {
    let limits = ModelLimits::gpt4o();

    assert!(!limits.should_compact(50_000));
    assert!(limits.should_compact(100_000));
    assert!(limits.should_compact(150_000));
}

#[test]
fn test_context_manager_record_and_retrieve() {
    let mut cm = ContextManager::new();

    cm.record(ResponseItem::user_message("Hello"));
    cm.record(ResponseItem::assistant_message("Hi there!"));

    assert_eq!(cm.len(), 2);
    assert!(!cm.is_empty());
}

#[test]
fn test_context_manager_token_estimation() {
    let mut cm = ContextManager::new();

    // ~3 tokens each = ~6 total
    cm.record(ResponseItem::user_message("hello world"));
    cm.record(ResponseItem::assistant_message("hello there"));

    let tokens = cm.estimate_total_tokens();
    assert!(tokens >= 4 && tokens <= 10);
}

#[test]
fn test_context_manager_orphan_removal() {
    let mut cm = ContextManager::new();

    // Function call
    cm.record(ResponseItem::function_call("call_1", "read_file", serde_json::json!({})));
    // Matching output
    cm.record(ResponseItem::function_output("call_1", "file content", false));
    // Orphan output (no matching call)
    cm.record(ResponseItem::function_output("call_2", "orphan", false));

    let history = cm.get_history_for_prompt();
    assert_eq!(history.len(), 2); // call_1 and its output only
}

#[test]
fn test_context_manager_truncate_to_fit() {
    let mut cm = ContextManager::new();

    // Add many messages
    for i in 0..20 {
        cm.record(ResponseItem::user_message(format!("message {}", i)));
    }

    let initial_tokens = cm.estimate_total_tokens();
    cm.truncate_to_fit(initial_tokens / 2);

    assert!(cm.estimate_total_tokens() <= initial_tokens / 2 + 10); // Some tolerance
    assert!(cm.len() < 20);
}

#[test]
fn test_context_manager_preserve_system_message() {
    let mut cm = ContextManager::new();

    cm.record(ResponseItem::system("You are a helpful assistant"));
    cm.record(ResponseItem::user_message("Hello"));
    cm.record(ResponseItem::assistant_message("Hi!"));

    cm.truncate_to_fit(5); // Very low limit

    // System message should remain
    assert!(matches!(cm.items().first(), Some(ResponseItem::System(_))));
}

#[test]
fn test_context_manager_pending_function_calls() {
    let mut cm = ContextManager::new();

    cm.record(ResponseItem::function_call("call_1", "read_file", serde_json::json!({})));
    cm.record(ResponseItem::function_call("call_2", "write_file", serde_json::json!({})));
    cm.record(ResponseItem::function_output("call_1", "content", false));

    let pending = cm.pending_function_calls();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].call_id, "call_2");
}
