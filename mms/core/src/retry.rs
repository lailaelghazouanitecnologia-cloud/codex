//! Retry logic with exponential backoff for stream operations.
//!
//! Handles transient failures in LLM API calls with configurable
//! retry policies, jitter, and backoff strategies.

use std::time::Duration;
use tracing::{debug, warn};

/// Default retry configuration
pub const DEFAULT_MAX_RETRIES: u32 = 3;
pub const DEFAULT_INITIAL_DELAY_MS: u64 = 1000;
pub const DEFAULT_MAX_DELAY_MS: u64 = 30000;
pub const DEFAULT_MULTIPLIER: f64 = 2.0;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Whether to add jitter to delays
    pub jitter: bool,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_delay: Duration::from_millis(DEFAULT_INITIAL_DELAY_MS),
            max_delay: Duration::from_millis(DEFAULT_MAX_DELAY_MS),
            multiplier: DEFAULT_MULTIPLIER,
            jitter: true,
            jitter_factor: 0.25,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with the given max retries
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Set the initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set the max delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set the multiplier
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Enable or disable jitter
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Calculate delay for the given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay_ms = self.initial_delay.as_millis() as f64
            * self.multiplier.powi(attempt as i32);

        let capped_delay_ms = base_delay_ms.min(self.max_delay.as_millis() as f64);

        let final_delay_ms = if self.jitter {
            // Add random jitter within the jitter factor range
            let jitter_range = capped_delay_ms * self.jitter_factor;
            let jitter = (rand_simple() * 2.0 - 1.0) * jitter_range;
            (capped_delay_ms + jitter).max(0.0)
        } else {
            capped_delay_ms
        };

        Duration::from_millis(final_delay_ms as u64)
    }

    /// Check if an error is retryable based on its type
    pub fn is_retryable(&self, error: &RetryableError) -> bool {
        error.is_retryable()
    }
}

/// Simple pseudo-random number generator (0.0 to 1.0)
/// Uses thread-local state for cheap randomness
fn rand_simple() -> f64 {
    use std::cell::Cell;
    use std::time::SystemTime;

    thread_local! {
        static STATE: Cell<u64> = Cell::new(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(12345)
        );
    }

    STATE.with(|state| {
        // Simple LCG
        let s = state.get();
        let new_state = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        state.set(new_state);
        (new_state as f64) / (u64::MAX as f64)
    })
}

/// Categories of retryable errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryableError {
    /// Network timeout
    Timeout,
    /// Connection failed
    ConnectionFailed,
    /// Rate limited (with optional retry-after)
    RateLimited(Option<u64>),
    /// Server error (5xx)
    ServerError(u16),
    /// Stream interrupted
    StreamInterrupted,
    /// Non-retryable error
    NonRetryable,
}

impl RetryableError {
    /// Check if this error type is retryable
    pub fn is_retryable(&self) -> bool {
        !matches!(self, Self::NonRetryable)
    }

    /// Get suggested delay for rate limit errors
    pub fn suggested_delay(&self) -> Option<Duration> {
        match self {
            Self::RateLimited(Some(secs)) => Some(Duration::from_secs(*secs)),
            Self::RateLimited(None) => Some(Duration::from_secs(5)),
            _ => None,
        }
    }

    /// Parse from HTTP status code
    pub fn from_status(status: u16) -> Self {
        match status {
            408 => Self::Timeout,
            429 => Self::RateLimited(None),
            500..=599 => Self::ServerError(status),
            _ => Self::NonRetryable,
        }
    }
}

/// State tracker for retry attempts
#[derive(Debug)]
pub struct RetryState {
    config: RetryConfig,
    attempt: u32,
    last_error: Option<RetryableError>,
}

impl RetryState {
    /// Create a new retry state
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            attempt: 0,
            last_error: None,
        }
    }

    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Check if we should retry
    pub fn should_retry(&self, error: &RetryableError) -> bool {
        error.is_retryable() && self.attempt < self.config.max_retries
    }

    /// Record an error and get the delay before next retry
    pub fn record_error(&mut self, error: RetryableError) -> Option<Duration> {
        if !self.should_retry(&error) {
            return None;
        }

        self.last_error = Some(error);

        // Use suggested delay for rate limits, otherwise exponential backoff
        let delay = error
            .suggested_delay()
            .unwrap_or_else(|| self.config.delay_for_attempt(self.attempt));

        self.attempt += 1;

        debug!(
            attempt = self.attempt,
            max_retries = self.config.max_retries,
            delay_ms = delay.as_millis() as u64,
            "Scheduling retry"
        );

        Some(delay)
    }

    /// Record a successful attempt (resets retry count)
    pub fn record_success(&mut self) {
        self.attempt = 0;
        self.last_error = None;
    }

    /// Get current attempt number
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Get remaining retries
    pub fn remaining_retries(&self) -> u32 {
        self.config.max_retries.saturating_sub(self.attempt)
    }
}

/// Execute an async operation with retries
pub async fn with_retry<F, Fut, T, E>(
    config: RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Into<RetryableError> + Clone,
{
    let mut state = RetryState::new(config);

    loop {
        match operation().await {
            Ok(result) => {
                state.record_success();
                return Ok(result);
            }
            Err(e) => {
                let retryable = e.clone().into();
                if let Some(delay) = state.record_error(retryable) {
                    warn!(
                        attempt = state.attempt(),
                        delay_ms = delay.as_millis() as u64,
                        "Operation failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

/// Execute with retries and cancellation support
pub async fn with_retry_cancellable<F, Fut, T, E>(
    config: RetryConfig,
    cancel_token: &crate::cancel::CancellationToken,
    mut operation: F,
) -> Result<T, RetryError<E>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Into<RetryableError> + Clone,
{
    let mut state = RetryState::new(config);

    loop {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Err(RetryError::Cancelled);
        }

        match cancel_token.run(operation()).await {
            None => return Err(RetryError::Cancelled),
            Some(Ok(result)) => {
                state.record_success();
                return Ok(result);
            }
            Some(Err(e)) => {
                let retryable = e.clone().into();
                if let Some(delay) = state.record_error(retryable) {
                    // Sleep with cancellation support
                    if cancel_token.run(tokio::time::sleep(delay)).await.is_none() {
                        return Err(RetryError::Cancelled);
                    }
                } else {
                    return Err(RetryError::Failed(e));
                }
            }
        }
    }
}

/// Error type for retry operations
#[derive(Debug)]
pub enum RetryError<E> {
    /// Operation was cancelled
    Cancelled,
    /// Operation failed after all retries
    Failed(E),
    /// Maximum retries exceeded
    MaxRetriesExceeded(E),
}

impl<E: std::fmt::Display> std::fmt::Display for RetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "operation cancelled"),
            Self::Failed(e) => write!(f, "operation failed: {}", e),
            Self::MaxRetriesExceeded(e) => write!(f, "max retries exceeded: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RetryError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Failed(e) | Self::MaxRetriesExceeded(e) => Some(e),
            Self::Cancelled => None,
        }
    }
}
