//! Token counting and estimation utilities.
//!
//! Uses byte-based heuristics for fast approximate token counting.
//! The standard heuristic is ~4 bytes per token for English text.

use serde::{Deserialize, Serialize};

/// Average bytes per token (conservative estimate for English)
const BYTES_PER_TOKEN: f64 = 4.0;

/// Multiplier for reasoning/encrypted content (typically more compact)
const REASONING_COMPRESSION_RATIO: f64 = 0.75;

/// Token usage information for a turn or session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsageInfo {
    /// Input/prompt tokens
    pub input_tokens: Option<u64>,
    /// Output/completion tokens
    pub output_tokens: Option<u64>,
    /// Cached input tokens (prompt cache hits)
    pub cache_input_tokens: Option<u64>,
    /// Cache write tokens
    pub cache_write_tokens: Option<u64>,
    /// Total tokens (input + output)
    pub total_tokens: Option<u64>,
}

impl TokenUsageInfo {
    /// Create a new token usage info
    pub fn new(input: u64, output: u64) -> Self {
        Self {
            input_tokens: Some(input),
            output_tokens: Some(output),
            cache_input_tokens: None,
            cache_write_tokens: None,
            total_tokens: Some(input + output),
        }
    }

    /// Create from optional values
    pub fn from_optional(input: Option<u64>, output: Option<u64>) -> Self {
        let total = match (input, output) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        };
        Self {
            input_tokens: input,
            output_tokens: output,
            cache_input_tokens: None,
            cache_write_tokens: None,
            total_tokens: total,
        }
    }

    /// Add cache information
    pub fn with_cache(mut self, cache_input: u64, cache_write: u64) -> Self {
        self.cache_input_tokens = Some(cache_input);
        self.cache_write_tokens = Some(cache_write);
        self
    }

    /// Merge with another TokenUsageInfo (accumulate)
    pub fn merge(&mut self, other: &TokenUsageInfo) {
        if let Some(input) = other.input_tokens {
            *self.input_tokens.get_or_insert(0) += input;
        }
        if let Some(output) = other.output_tokens {
            *self.output_tokens.get_or_insert(0) += output;
        }
        if let Some(cache_input) = other.cache_input_tokens {
            *self.cache_input_tokens.get_or_insert(0) += cache_input;
        }
        if let Some(cache_write) = other.cache_write_tokens {
            *self.cache_write_tokens.get_or_insert(0) += cache_write;
        }
        // Recalculate total
        self.total_tokens = match (self.input_tokens, self.output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            (Some(i), None) => Some(i),
            (None, Some(o)) => Some(o),
            (None, None) => None,
        };
    }

    /// Get total tokens
    pub fn total(&self) -> u64 {
        self.total_tokens.unwrap_or(0)
    }

    /// Check if we have any token info
    pub fn has_info(&self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some()
    }
}

/// Estimate token count from a string using byte heuristic
pub fn estimate_tokens(text: &str) -> u64 {
    approx_tokens_from_bytes(text.len())
}

/// Estimate tokens from byte count
pub fn approx_tokens_from_bytes(bytes: usize) -> u64 {
    (bytes as f64 / BYTES_PER_TOKEN).ceil() as u64
}

/// Estimate bytes from token count
pub fn approx_bytes_from_tokens(tokens: u64) -> usize {
    (tokens as f64 * BYTES_PER_TOKEN) as usize
}

/// Estimate tokens for reasoning/encrypted content (more compact)
pub fn estimate_reasoning_tokens(encrypted_bytes: usize) -> u64 {
    ((encrypted_bytes as f64 / BYTES_PER_TOKEN) * REASONING_COMPRESSION_RATIO).ceil() as u64
}

/// Rate limit snapshot from the API
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimitSnapshot {
    /// Requests per minute limit
    pub requests_limit: Option<u64>,
    /// Requests remaining
    pub requests_remaining: Option<u64>,
    /// Tokens per minute limit
    pub tokens_limit: Option<u64>,
    /// Tokens remaining
    pub tokens_remaining: Option<u64>,
    /// Time until reset (seconds)
    pub reset_seconds: Option<f64>,
}

impl RateLimitSnapshot {
    /// Check if we're close to the rate limit
    pub fn is_near_limit(&self) -> bool {
        if let (Some(remaining), Some(limit)) = (self.requests_remaining, self.requests_limit) {
            if limit > 0 && (remaining as f64 / limit as f64) < 0.1 {
                return true;
            }
        }
        if let (Some(remaining), Some(limit)) = (self.tokens_remaining, self.tokens_limit) {
            if limit > 0 && (remaining as f64 / limit as f64) < 0.1 {
                return true;
            }
        }
        false
    }

    /// Get suggested delay if near limit
    pub fn suggested_delay(&self) -> Option<std::time::Duration> {
        if self.is_near_limit() {
            if let Some(reset) = self.reset_seconds {
                return Some(std::time::Duration::from_secs_f64(reset));
            }
            // Default backoff if no reset time
            return Some(std::time::Duration::from_secs(5));
        }
        None
    }
}
