//! Tests for the cancellation token system

use std::time::Duration;
use mms_core::cancel::{CancellationToken, CancelOnDrop, CancelledError};

#[tokio::test]
async fn test_cancellation_token_new_not_cancelled() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
}

#[tokio::test]
async fn test_cancellation_token_cancel() {
    let token = CancellationToken::new();
    token.cancel();
    assert!(token.is_cancelled());
}

#[tokio::test]
async fn test_cancellation_token_cancelled_returns_immediately() {
    let token = CancellationToken::new();
    token.cancel();

    // Should return immediately since already cancelled
    let start = std::time::Instant::now();
    token.cancelled().await;
    assert!(start.elapsed() < Duration::from_millis(10));
}

#[tokio::test]
async fn test_cancellation_token_cancelled_waits() {
    let token = CancellationToken::new();
    let token_clone = token.clone();

    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        token_clone.cancel();
    });

    let start = std::time::Instant::now();
    token.cancelled().await;
    assert!(start.elapsed() >= Duration::from_millis(40));

    handle.await.unwrap();
}

#[tokio::test]
async fn test_cancellation_token_run_completes() {
    let token = CancellationToken::new();

    let result = token.run(async { 42 }).await;
    assert_eq!(result, Some(42));
}

#[tokio::test]
async fn test_cancellation_token_run_cancelled() {
    let token = CancellationToken::new();
    token.cancel();

    let result = token.run(async { 42 }).await;
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_cancellation_token_run_result() {
    let token = CancellationToken::new();

    let result = token.run_result(async { 42 }).await;
    assert_eq!(result, Ok(42));

    token.cancel();
    let result = token.run_result(async { 42 }).await;
    assert_eq!(result, Err(CancelledError));
}

#[tokio::test]
async fn test_child_token_parent_cancel() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    assert!(!child.is_cancelled());
    parent.cancel();
    assert!(child.is_cancelled());
}

#[tokio::test]
async fn test_child_token_child_cancel() {
    let parent = CancellationToken::new();
    let child = parent.child_token();

    assert!(!child.is_cancelled());
    child.cancel();
    assert!(child.is_cancelled());
    // Parent should not be cancelled
    assert!(!parent.is_cancelled());
}

#[tokio::test]
async fn test_cancel_on_drop() {
    let token = CancellationToken::new();

    {
        let _guard = CancelOnDrop::new(token.clone());
        assert!(!token.is_cancelled());
    }

    // Token should be cancelled after guard is dropped
    assert!(token.is_cancelled());
}

#[tokio::test]
async fn test_cancel_on_drop_disarm() {
    let token = CancellationToken::new();

    {
        let guard = CancelOnDrop::new(token.clone());
        guard.disarm();
    }

    // Token should NOT be cancelled since guard was disarmed
    assert!(!token.is_cancelled());
}

#[tokio::test]
async fn test_multiple_cancel_calls() {
    let token = CancellationToken::new();

    token.cancel();
    token.cancel();  // Should be idempotent
    token.cancel();

    assert!(token.is_cancelled());
}

#[test]
fn test_cancelled_error_display() {
    let err = CancelledError;
    assert_eq!(err.to_string(), "operation cancelled");
}
