//! Cancellation token for cooperative task interruption.
//!
//! This module provides a structured way to signal and handle cancellation
//! across async tasks, similar to CancellationToken in C# or Go's context.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// A token used to signal cancellation to cooperating tasks.
///
/// Cancellation is cooperative - tasks must check the token periodically
/// or select on `cancelled()` to respond to cancellation requests.
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationInner>,
}

struct CancellationInner {
    /// Whether cancellation has been requested
    cancelled: AtomicBool,
    /// Notifier for async waiters
    notify: Notify,
}

impl CancellationToken {
    /// Create a new cancellation token
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationInner {
                cancelled: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    /// Request cancellation
    ///
    /// This will set the cancelled flag and wake all waiters.
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::SeqCst) {
            // Only notify if we actually changed the state
            self.inner.notify.notify_waiters();
        }
    }

    /// Wait for cancellation asynchronously
    ///
    /// Returns immediately if already cancelled, otherwise waits
    /// until `cancel()` is called.
    pub async fn cancelled(&self) {
        // Fast path - already cancelled
        if self.is_cancelled() {
            return;
        }

        // Wait for notification
        loop {
            let notified = self.inner.notify.notified();

            // Check again after setting up notified
            if self.is_cancelled() {
                return;
            }

            notified.await;

            // Check after wakeup
            if self.is_cancelled() {
                return;
            }
        }
    }

    /// Create a child token that is cancelled when either this token
    /// or the child's own cancel() is called.
    pub fn child_token(&self) -> ChildCancellationToken {
        ChildCancellationToken {
            parent: self.clone(),
            local: CancellationToken::new(),
        }
    }

    /// Run a future with cancellation support
    ///
    /// Returns None if cancelled before completion
    pub async fn run<F, T>(&self, future: F) -> Option<T>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::select! {
            biased;
            _ = self.cancelled() => None,
            result = future => Some(result),
        }
    }

    /// Run a future with cancellation, returning a Result
    pub async fn run_result<F, T>(&self, future: F) -> Result<T, CancelledError>
    where
        F: std::future::Future<Output = T>,
    {
        self.run(future).await.ok_or(CancelledError)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// A child cancellation token that can be cancelled independently
/// but is also cancelled when its parent is cancelled.
#[derive(Clone)]
pub struct ChildCancellationToken {
    parent: CancellationToken,
    local: CancellationToken,
}

impl ChildCancellationToken {
    /// Check if this token (or its parent) is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.local.is_cancelled() || self.parent.is_cancelled()
    }

    /// Cancel this child token (does not cancel parent)
    pub fn cancel(&self) {
        self.local.cancel();
    }

    /// Wait for cancellation (either local or parent)
    pub async fn cancelled(&self) {
        tokio::select! {
            biased;
            _ = self.parent.cancelled() => {},
            _ = self.local.cancelled() => {},
        }
    }

    /// Run a future with cancellation support
    pub async fn run<F, T>(&self, future: F) -> Option<T>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::select! {
            biased;
            _ = self.cancelled() => None,
            result = future => Some(result),
        }
    }
}

/// Error returned when an operation is cancelled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelledError;

impl std::fmt::Display for CancelledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation cancelled")
    }
}

impl std::error::Error for CancelledError {}

/// Guard that cancels a token when dropped
pub struct CancelOnDrop {
    token: CancellationToken,
}

impl CancelOnDrop {
    /// Create a guard that will cancel the given token on drop
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }

    /// Disarm the guard - token will not be cancelled on drop
    pub fn disarm(self) {
        std::mem::forget(self);
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

/// Extension trait for running futures with timeout and cancellation
pub trait CancellableFuture {
    /// The output type of the future
    type Output;

    /// Run with both timeout and cancellation token
    fn with_cancellation(
        self,
        token: &CancellationToken,
    ) -> impl std::future::Future<Output = Option<Self::Output>>;

    /// Run with timeout and cancellation, returning Result
    fn with_cancellation_result(
        self,
        token: &CancellationToken,
    ) -> impl std::future::Future<Output = Result<Self::Output, CancelledError>>;
}

impl<F, T> CancellableFuture for F
where
    F: std::future::Future<Output = T>,
{
    type Output = T;

    async fn with_cancellation(self, token: &CancellationToken) -> Option<T> {
        token.run(self).await
    }

    async fn with_cancellation_result(
        self,
        token: &CancellationToken,
    ) -> Result<T, CancelledError> {
        token.run_result(self).await
    }
}
