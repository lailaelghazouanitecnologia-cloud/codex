//! Tests for parallel tool execution

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mms_core::parallel::{
    ParallelConfig, ParallelExecutor, DependentToolCall, ToolScheduler,
    batch_by_dependency, execute_parallel,
};
use mms_core::cancel::CancellationToken;

#[test]
fn test_parallel_config_default() {
    let config = ParallelConfig::default();
    assert_eq!(config.max_concurrent, 5);
    assert!(!config.fail_fast);
    assert!(!config.preserve_order);
}

#[test]
fn test_parallel_config_builder() {
    let config = ParallelConfig::new(10)
        .with_fail_fast(true)
        .with_preserve_order(true);

    assert_eq!(config.max_concurrent, 10);
    assert!(config.fail_fast);
    assert!(config.preserve_order);
}

#[tokio::test]
async fn test_parallel_executor_empty() {
    let executor = ParallelExecutor::default_config();
    let results: Vec<_> = executor
        .execute(vec![], |_call_id, _idx| async { 42 })
        .await;

    assert!(results.is_empty());
}

#[tokio::test]
async fn test_parallel_executor_single() {
    let executor = ParallelExecutor::default_config();
    let results = executor
        .execute(vec!["call1".to_string()], |call_id, idx| async move {
            (call_id, idx)
        })
        .await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].call_id, "call1");
    assert!(results[0].result.is_some());
    assert!(!results[0].cancelled);
}

#[tokio::test]
async fn test_parallel_executor_multiple() {
    let executor = ParallelExecutor::new(ParallelConfig::new(5));
    let call_ids: Vec<_> = (0..10).map(|i| format!("call{}", i)).collect();

    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let results = executor
        .execute(call_ids, move |_call_id, _idx| {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                42
            }
        })
        .await;

    assert_eq!(results.len(), 10);
    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn test_parallel_executor_respects_concurrency_limit() {
    let executor = ParallelExecutor::new(ParallelConfig::new(2));
    let call_ids: Vec<_> = (0..4).map(|i| format!("call{}", i)).collect();

    let max_concurrent = Arc::new(AtomicU32::new(0));
    let current_concurrent = Arc::new(AtomicU32::new(0));

    let max_clone = max_concurrent.clone();
    let current_clone = current_concurrent.clone();

    let results = executor
        .execute(call_ids, move |_call_id, _idx| {
            let max = max_clone.clone();
            let current = current_clone.clone();
            async move {
                let running = current.fetch_add(1, Ordering::SeqCst) + 1;
                max.fetch_max(running, Ordering::SeqCst);

                tokio::time::sleep(Duration::from_millis(50)).await;

                current.fetch_sub(1, Ordering::SeqCst);
                42
            }
        })
        .await;

    assert_eq!(results.len(), 4);
    // Max concurrent should not exceed limit
    assert!(max_concurrent.load(Ordering::SeqCst) <= 2);
}

#[tokio::test]
async fn test_parallel_executor_preserve_order() {
    let config = ParallelConfig::new(5).with_preserve_order(true);
    let executor = ParallelExecutor::new(config);
    let call_ids: Vec<_> = (0..5).map(|i| format!("call{}", i)).collect();

    let results = executor
        .execute(call_ids, |_call_id, idx| async move {
            // Random delay to mix up completion order
            tokio::time::sleep(Duration::from_millis((50 - idx * 10) as u64)).await;
            idx
        })
        .await;

    // Results should be in original order
    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.index, i);
    }
}

#[tokio::test]
async fn test_parallel_executor_with_cancellation() {
    let executor = ParallelExecutor::new(ParallelConfig::new(2));
    let call_ids: Vec<_> = (0..4).map(|i| format!("call{}", i)).collect();

    let cancel_token = CancellationToken::new();
    let token_clone = cancel_token.clone();

    // Cancel after a short delay
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        token_clone.cancel();
    });

    let results = executor
        .execute_with_cancellation(
            call_ids,
            |_call_id, _idx| async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                42
            },
            &cancel_token,
        )
        .await;

    // Some should have been cancelled
    let cancelled = results.iter().filter(|r| r.cancelled).count();
    assert!(cancelled > 0, "Expected some cancellations");
}

#[tokio::test]
async fn test_execute_parallel_simple() {
    let call_ids: Vec<_> = (0..3).map(|i| format!("call{}", i)).collect();

    let results: Vec<Result<i32, ()>> = execute_parallel(
        call_ids,
        5,
        |_call_id, idx| async move { Ok(idx as i32) },
    )
    .await;

    assert_eq!(results.len(), 3);
    // Results should be in order
    assert_eq!(results[0], Ok(0));
    assert_eq!(results[1], Ok(1));
    assert_eq!(results[2], Ok(2));
}

#[test]
fn test_batch_by_dependency_empty() {
    let items: Vec<i32> = vec![];
    let batches = batch_by_dependency(items, |_a, _b| false);
    assert!(batches.is_empty());
}

#[test]
fn test_batch_by_dependency_no_deps() {
    let items = vec![1, 2, 3, 4, 5];
    let batches = batch_by_dependency(items, |_a, _b| false);

    // All in one batch (no dependencies)
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 5);
}

#[test]
fn test_batch_by_dependency_linear() {
    // Each item depends on the previous
    let items = vec![1, 2, 3];
    let batches = batch_by_dependency(items, |a, b| *a > *b);

    // Should be three batches (sequential)
    assert_eq!(batches.len(), 3);
    assert_eq!(batches[0], vec![1]);
    assert_eq!(batches[1], vec![2]);
    assert_eq!(batches[2], vec![3]);
}

#[test]
fn test_dependent_tool_call() {
    let call = DependentToolCall::new("call1", "shell")
        .with_dependency("dep1")
        .with_dependencies(vec!["dep2".to_string(), "dep3".to_string()]);

    assert_eq!(call.call_id, "call1");
    assert_eq!(call.tool_name, "shell");
    assert_eq!(call.depends_on.len(), 3);
}

#[test]
fn test_tool_scheduler_basic() {
    let calls = vec![
        DependentToolCall::new("a", "tool"),
        DependentToolCall::new("b", "tool").with_dependency("a"),
        DependentToolCall::new("c", "tool").with_dependency("a"),
        DependentToolCall::new("d", "tool").with_dependency("b").with_dependency("c"),
    ];

    let mut scheduler = ToolScheduler::new(calls);

    // First batch: only 'a' has no dependencies
    let batch1 = scheduler.next_batch();
    assert_eq!(batch1.len(), 1);
    assert_eq!(batch1[0].call_id, "a");

    // Complete 'a'
    scheduler.complete("a", true);

    // Second batch: 'b' and 'c' now available
    let batch2 = scheduler.next_batch();
    assert_eq!(batch2.len(), 2);

    scheduler.complete("b", true);
    scheduler.complete("c", true);

    // Third batch: 'd' now available
    let batch3 = scheduler.next_batch();
    assert_eq!(batch3.len(), 1);
    assert_eq!(batch3[0].call_id, "d");

    scheduler.complete("d", true);

    assert!(scheduler.is_complete());
    assert!(scheduler.remaining().is_empty());
}

#[test]
fn test_tool_scheduler_incomplete() {
    let calls = vec![
        DependentToolCall::new("a", "tool"),
        DependentToolCall::new("b", "tool").with_dependency("a"),
    ];

    let scheduler = ToolScheduler::new(calls);

    assert!(!scheduler.is_complete());
    assert_eq!(scheduler.remaining().len(), 2);
}
