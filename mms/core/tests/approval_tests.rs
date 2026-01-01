//! Tests for the approval system

use std::time::Duration;
use mms_core::approval::{
    new_shared_manager, request_command_approval, approve, reject, cancel_all, ReviewDecision,
};

#[tokio::test]
async fn test_approval_flow() {
    let manager = new_shared_manager();
    let request_id = "test_request".to_string();

    // Create request in background task
    let mgr_clone = manager.clone();
    let rid = request_id.clone();
    let handle = tokio::spawn(async move {
        request_command_approval(
            &mgr_clone,
            rid,
            "shell".to_string(),
            vec!["ls".to_string()],
            "/tmp".to_string(),
            Duration::from_secs(10),
        )
        .await
    });

    // Give the task time to create the request
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Approve
    let delivered = approve(&manager, &request_id).await;
    assert!(delivered);

    // Check result
    let decision = handle.await.unwrap();
    assert_eq!(decision, ReviewDecision::Approved);
}

#[tokio::test]
async fn test_rejection_flow() {
    let manager = new_shared_manager();
    let request_id = "test_reject".to_string();

    let mgr_clone = manager.clone();
    let rid = request_id.clone();
    let handle = tokio::spawn(async move {
        request_command_approval(
            &mgr_clone,
            rid,
            "shell".to_string(),
            vec!["rm".to_string(), "-rf".to_string()],
            "/".to_string(),
            Duration::from_secs(10),
        )
        .await
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    let delivered = reject(&manager, &request_id).await;
    assert!(delivered);

    let decision = handle.await.unwrap();
    assert_eq!(decision, ReviewDecision::Rejected);
}

#[tokio::test]
async fn test_timeout() {
    let manager = new_shared_manager();
    let request_id = "test_timeout".to_string();

    let decision = request_command_approval(
        &manager,
        request_id,
        "shell".to_string(),
        vec!["ls".to_string()],
        "/tmp".to_string(),
        Duration::from_millis(50), // Very short timeout
    )
    .await;

    assert_eq!(decision, ReviewDecision::TimedOut);
}

#[tokio::test]
async fn test_cancel_all() {
    let manager = new_shared_manager();

    // Create multiple requests
    let mgr_clone = manager.clone();
    let handle1 = tokio::spawn(async move {
        request_command_approval(
            &mgr_clone,
            "req1".to_string(),
            "shell".to_string(),
            vec![],
            "/".to_string(),
            Duration::from_secs(10),
        )
        .await
    });

    let mgr_clone = manager.clone();
    let handle2 = tokio::spawn(async move {
        request_command_approval(
            &mgr_clone,
            "req2".to_string(),
            "shell".to_string(),
            vec![],
            "/".to_string(),
            Duration::from_secs(10),
        )
        .await
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    // Cancel all
    cancel_all(&manager).await;

    // Both should be cancelled
    assert_eq!(handle1.await.unwrap(), ReviewDecision::Cancelled);
    assert_eq!(handle2.await.unwrap(), ReviewDecision::Cancelled);
}

#[test]
fn test_review_decision() {
    assert!(ReviewDecision::Approved.should_execute());
    assert!(!ReviewDecision::Rejected.should_execute());
    assert!(!ReviewDecision::TimedOut.should_execute());
    assert!(!ReviewDecision::Cancelled.should_execute());
}
