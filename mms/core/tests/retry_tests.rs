//! Tests for the retry system

use std::time::Duration;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use mms_core::retry::{RetryConfig, RetryState, RetryableError, with_retry};

#[test]
fn test_retry_config_default() {
    let config = RetryConfig::default();
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.initial_delay, Duration::from_millis(1000));
    assert_eq!(config.max_delay, Duration::from_millis(30000));
    assert_eq!(config.multiplier, 2.0);
    assert!(config.jitter);
}

#[test]
fn test_retry_config_builder() {
    let config = RetryConfig::new(5)
        .with_initial_delay(Duration::from_millis(500))
        .with_max_delay(Duration::from_secs(10))
        .with_multiplier(3.0)
        .with_jitter(false);

    assert_eq!(config.max_retries, 5);
    assert_eq!(config.initial_delay, Duration::from_millis(500));
    assert_eq!(config.max_delay, Duration::from_secs(10));
    assert_eq!(config.multiplier, 3.0);
    assert!(!config.jitter);
}

#[test]
fn test_delay_for_attempt_no_jitter() {
    let config = RetryConfig::new(5)
        .with_initial_delay(Duration::from_millis(100))
        .with_multiplier(2.0)
        .with_jitter(false);

    assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
    assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
    assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
}

#[test]
fn test_delay_caps_at_max() {
    let config = RetryConfig::new(5)
        .with_initial_delay(Duration::from_millis(1000))
        .with_max_delay(Duration::from_millis(2000))
        .with_multiplier(2.0)
        .with_jitter(false);

    // 1000, 2000, 2000 (capped), 2000 (capped)
    assert_eq!(config.delay_for_attempt(0), Duration::from_millis(1000));
    assert_eq!(config.delay_for_attempt(1), Duration::from_millis(2000));
    assert_eq!(config.delay_for_attempt(2), Duration::from_millis(2000));
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(2000));
}

#[test]
fn test_retryable_error_is_retryable() {
    assert!(RetryableError::Timeout.is_retryable());
    assert!(RetryableError::ConnectionFailed.is_retryable());
    assert!(RetryableError::RateLimited(None).is_retryable());
    assert!(RetryableError::ServerError(500).is_retryable());
    assert!(RetryableError::StreamInterrupted.is_retryable());
    assert!(!RetryableError::NonRetryable.is_retryable());
}

#[test]
fn test_retryable_error_from_status() {
    assert_eq!(RetryableError::from_status(408), RetryableError::Timeout);
    assert_eq!(RetryableError::from_status(429), RetryableError::RateLimited(None));
    assert_eq!(RetryableError::from_status(500), RetryableError::ServerError(500));
    assert_eq!(RetryableError::from_status(502), RetryableError::ServerError(502));
    assert_eq!(RetryableError::from_status(400), RetryableError::NonRetryable);
    assert_eq!(RetryableError::from_status(404), RetryableError::NonRetryable);
}

#[test]
fn test_retryable_error_suggested_delay() {
    assert_eq!(
        RetryableError::RateLimited(Some(10)).suggested_delay(),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        RetryableError::RateLimited(None).suggested_delay(),
        Some(Duration::from_secs(5))
    );
    assert_eq!(RetryableError::Timeout.suggested_delay(), None);
}

#[test]
fn test_retry_state_should_retry() {
    let mut state = RetryState::new(RetryConfig::new(2));

    assert!(state.should_retry(&RetryableError::Timeout));
    state.record_error(RetryableError::Timeout);
    assert!(state.should_retry(&RetryableError::Timeout));
    state.record_error(RetryableError::Timeout);
    assert!(!state.should_retry(&RetryableError::Timeout)); // Exhausted
}

#[test]
fn test_retry_state_not_retry_non_retryable() {
    let state = RetryState::new(RetryConfig::new(5));
    assert!(!state.should_retry(&RetryableError::NonRetryable));
}

#[test]
fn test_retry_state_record_success_resets() {
    let mut state = RetryState::new(RetryConfig::new(3));

    state.record_error(RetryableError::Timeout);
    state.record_error(RetryableError::Timeout);
    assert_eq!(state.remaining_retries(), 1);

    state.record_success();
    assert_eq!(state.remaining_retries(), 3);
}

// Test error type for with_retry
#[derive(Clone, Debug)]
struct TestError(RetryableError);

impl From<TestError> for RetryableError {
    fn from(e: TestError) -> Self {
        e.0
    }
}

#[tokio::test]
async fn test_with_retry_succeeds_first_try() {
    let config = RetryConfig::new(3).with_jitter(false);
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result: Result<i32, TestError> = with_retry(config, || {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(42)
        }
    }).await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_with_retry_succeeds_after_failures() {
    let config = RetryConfig::new(3)
        .with_initial_delay(Duration::from_millis(10))
        .with_jitter(false);
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result: Result<i32, TestError> = with_retry(config, || {
        let c = counter_clone.clone();
        async move {
            let count = c.fetch_add(1, Ordering::SeqCst) + 1;
            if count < 3 {
                Err(TestError(RetryableError::Timeout))
            } else {
                Ok(42)
            }
        }
    }).await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_with_retry_exhausted() {
    let config = RetryConfig::new(2)
        .with_initial_delay(Duration::from_millis(10))
        .with_jitter(false);
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result: Result<i32, TestError> = with_retry(config, || {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err(TestError(RetryableError::Timeout))
        }
    }).await;

    assert!(result.is_err());
    // 1 initial + 2 retries = 3 total attempts
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_with_retry_non_retryable_fails_immediately() {
    let config = RetryConfig::new(3);
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let result: Result<i32, TestError> = with_retry(config, || {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err(TestError(RetryableError::NonRetryable))
        }
    }).await;

    assert!(result.is_err());
    assert_eq!(counter.load(Ordering::SeqCst), 1); // Only one attempt
}
