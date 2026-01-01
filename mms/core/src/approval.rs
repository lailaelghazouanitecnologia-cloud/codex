//! Approval system for tool execution gating.
//!
//! This module implements:
//! - Pending approval tracking
//! - Oneshot channel-based request/response matching
//! - Timeout handling
//! - Approval/rejection decision routing

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{oneshot, Mutex};
use tracing::{debug, warn};


/// Default timeout for approval requests (2 minutes)
pub const DEFAULT_APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

/// Decision on an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewDecision {
    /// User approved the action
    Approved,
    /// User rejected the action
    #[default]
    Rejected,
    /// Request timed out
    TimedOut,
    /// Request was cancelled (e.g., session interrupted)
    Cancelled,
}

impl ReviewDecision {
    /// Check if the decision allows execution
    pub fn should_execute(&self) -> bool {
        matches!(self, Self::Approved)
    }

    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::TimedOut => "timed out",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Type of approval request
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalType {
    /// Command/tool execution approval
    Command {
        tool_name: String,
        command: Vec<String>,
        cwd: String,
    },
    /// File modification approval
    Patch {
        path: String,
        diff: String,
    },
}

/// An approval request waiting for user response
#[derive(Debug)]
pub struct PendingApproval {
    /// Unique request ID
    pub request_id: String,
    /// Type of approval
    pub approval_type: ApprovalType,
    /// Channel to send the decision
    tx: oneshot::Sender<ReviewDecision>,
}

/// Manager for pending approval requests
#[derive(Debug, Default)]
pub struct ApprovalManager {
    /// Map of request_id -> pending approval
    pending: HashMap<String, PendingApproval>,
}

impl ApprovalManager {
    /// Create a new approval manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an approval request and return the receiver
    pub fn create_request(
        &mut self,
        request_id: String,
        approval_type: ApprovalType,
    ) -> oneshot::Receiver<ReviewDecision> {
        let (tx, rx) = oneshot::channel();

        let pending = PendingApproval {
            request_id: request_id.clone(),
            approval_type,
            tx,
        };

        // Replace any existing pending request with the same ID
        if let Some(old) = self.pending.insert(request_id.clone(), pending) {
            // Cancel the old request
            let _ = old.tx.send(ReviewDecision::Cancelled);
            debug!(request_id = %request_id, "Replaced existing pending approval");
        }

        rx
    }

    /// Deliver a decision to a pending request
    pub fn deliver_decision(&mut self, request_id: &str, decision: ReviewDecision) -> bool {
        if let Some(pending) = self.pending.remove(request_id) {
            debug!(
                request_id = %request_id,
                decision = ?decision,
                "Delivering approval decision"
            );
            pending.tx.send(decision).is_ok()
        } else {
            warn!(request_id = %request_id, "No pending approval found for request");
            false
        }
    }

    /// Cancel all pending approvals
    pub fn cancel_all(&mut self) {
        for (request_id, pending) in self.pending.drain() {
            debug!(request_id = %request_id, "Cancelling pending approval");
            let _ = pending.tx.send(ReviewDecision::Cancelled);
        }
    }

    /// Get the number of pending approvals
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if there are any pending approvals
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Get all pending request IDs
    pub fn pending_ids(&self) -> Vec<&str> {
        self.pending.keys().map(|s| s.as_str()).collect()
    }
}

/// Thread-safe approval manager wrapped in Arc<Mutex>
pub type SharedApprovalManager = Arc<Mutex<ApprovalManager>>;

/// Create a new shared approval manager
pub fn new_shared_manager() -> SharedApprovalManager {
    Arc::new(Mutex::new(ApprovalManager::new()))
}

/// Request approval for a command and wait for decision
pub async fn request_command_approval(
    manager: &SharedApprovalManager,
    request_id: String,
    tool_name: String,
    command: Vec<String>,
    cwd: String,
    timeout: Duration,
) -> ReviewDecision {
    let rx = {
        let mut mgr = manager.lock().await;
        mgr.create_request(
            request_id.clone(),
            ApprovalType::Command {
                tool_name,
                command,
                cwd,
            },
        )
    };

    // Wait for decision with timeout
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(decision)) => decision,
        Ok(Err(_)) => {
            // Channel was closed (sender dropped)
            debug!(request_id = %request_id, "Approval channel closed");
            ReviewDecision::Cancelled
        }
        Err(_) => {
            // Timeout
            debug!(request_id = %request_id, "Approval request timed out");
            // Remove from pending
            let mut mgr = manager.lock().await;
            mgr.pending.remove(&request_id);
            ReviewDecision::TimedOut
        }
    }
}

/// Request approval for a file patch and wait for decision
pub async fn request_patch_approval(
    manager: &SharedApprovalManager,
    request_id: String,
    path: String,
    diff: String,
    timeout: Duration,
) -> ReviewDecision {
    let rx = {
        let mut mgr = manager.lock().await;
        mgr.create_request(
            request_id.clone(),
            ApprovalType::Patch { path, diff },
        )
    };

    // Wait for decision with timeout
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(decision)) => decision,
        Ok(Err(_)) => {
            debug!(request_id = %request_id, "Approval channel closed");
            ReviewDecision::Cancelled
        }
        Err(_) => {
            debug!(request_id = %request_id, "Approval request timed out");
            let mut mgr = manager.lock().await;
            mgr.pending.remove(&request_id);
            ReviewDecision::TimedOut
        }
    }
}

/// Deliver an approval decision
pub async fn approve(manager: &SharedApprovalManager, request_id: &str) -> bool {
    let mut mgr = manager.lock().await;
    mgr.deliver_decision(request_id, ReviewDecision::Approved)
}

/// Deliver a rejection decision
pub async fn reject(manager: &SharedApprovalManager, request_id: &str) -> bool {
    let mut mgr = manager.lock().await;
    mgr.deliver_decision(request_id, ReviewDecision::Rejected)
}

/// Cancel all pending approvals
pub async fn cancel_all(manager: &SharedApprovalManager) {
    let mut mgr = manager.lock().await;
    mgr.cancel_all();
}

