//! Truncation policies for managing context window limits.
//!
//! Supports both byte-based and token-based truncation modes.

use serde::{Deserialize, Serialize};

use super::token::{approx_bytes_from_tokens, approx_tokens_from_bytes};

/// Default byte limit for tool output
pub const DEFAULT_TOOL_OUTPUT_BYTES: usize = 50_000;

/// Default token limit for tool output
pub const DEFAULT_TOOL_OUTPUT_TOKENS: u64 = 12_500;

/// Policy for truncating content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruncationPolicy {
    /// Truncate based on byte count
    Bytes(usize),
    /// Truncate based on estimated token count
    Tokens(u64),
    /// No truncation
    None,
}

impl TruncationPolicy {
    /// Create a byte-based policy
    pub fn bytes(limit: usize) -> Self {
        Self::Bytes(limit)
    }

    /// Create a token-based policy
    pub fn tokens(limit: u64) -> Self {
        Self::Tokens(limit)
    }

    /// Create a policy from config, with fallback to model default
    pub fn from_config(config_limit: Option<u64>, model_default_bytes: usize) -> Self {
        if let Some(token_limit) = config_limit {
            Self::Tokens(token_limit)
        } else {
            Self::Bytes(model_default_bytes)
        }
    }

    /// Get the effective token budget
    pub fn token_budget(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => approx_tokens_from_bytes(*bytes),
            Self::Tokens(tokens) => *tokens,
            Self::None => u64::MAX,
        }
    }

    /// Get the effective byte budget
    pub fn byte_budget(&self) -> usize {
        match self {
            Self::Bytes(bytes) => *bytes,
            Self::Tokens(tokens) => approx_bytes_from_tokens(*tokens),
            Self::None => usize::MAX,
        }
    }

    /// Check if content exceeds this policy's limit
    pub fn exceeds(&self, content: &str) -> bool {
        match self {
            Self::Bytes(limit) => content.len() > *limit,
            Self::Tokens(limit) => approx_tokens_from_bytes(content.len()) > *limit,
            Self::None => false,
        }
    }

    /// Truncate content according to this policy
    pub fn truncate(&self, content: &str) -> TruncationResult {
        let original_bytes = content.len();

        match self {
            Self::None => TruncationResult {
                content: content.to_string(),
                original_bytes,
                truncated: false,
                truncated_bytes: 0,
            },
            Self::Bytes(limit) => {
                if content.len() <= *limit {
                    TruncationResult {
                        content: content.to_string(),
                        original_bytes,
                        truncated: false,
                        truncated_bytes: 0,
                    }
                } else {
                    let truncated = truncate_to_bytes(content, *limit);
                    TruncationResult {
                        truncated_bytes: original_bytes - truncated.len(),
                        content: truncated,
                        original_bytes,
                        truncated: true,
                    }
                }
            }
            Self::Tokens(limit) => {
                let byte_limit = approx_bytes_from_tokens(*limit);
                if content.len() <= byte_limit {
                    TruncationResult {
                        content: content.to_string(),
                        original_bytes,
                        truncated: false,
                        truncated_bytes: 0,
                    }
                } else {
                    let truncated = truncate_to_bytes(content, byte_limit);
                    TruncationResult {
                        truncated_bytes: original_bytes - truncated.len(),
                        content: truncated,
                        original_bytes,
                        truncated: true,
                    }
                }
            }
        }
    }

    /// Truncate with prefix/suffix preservation
    pub fn truncate_preserving_ends(&self, content: &str, prefix_ratio: f64) -> TruncationResult {
        let original_bytes = content.len();
        let limit = self.byte_budget();

        if content.len() <= limit {
            return TruncationResult {
                content: content.to_string(),
                original_bytes,
                truncated: false,
                truncated_bytes: 0,
            };
        }

        // Calculate prefix and suffix lengths
        let prefix_len = ((limit as f64) * prefix_ratio) as usize;
        let suffix_len = limit.saturating_sub(prefix_len).saturating_sub(50); // Reserve for marker

        let prefix = truncate_to_bytes(content, prefix_len);
        let suffix_start = content.len().saturating_sub(suffix_len);
        let suffix = &content[suffix_start..];

        let marker = format!("\n\n... [{} bytes truncated] ...\n\n", original_bytes - prefix.len() - suffix.len());
        let truncated = format!("{}{}{}", prefix, marker, suffix);

        TruncationResult {
            truncated_bytes: original_bytes - truncated.len(),
            content: truncated,
            original_bytes,
            truncated: true,
        }
    }
}

impl Default for TruncationPolicy {
    fn default() -> Self {
        Self::Bytes(DEFAULT_TOOL_OUTPUT_BYTES)
    }
}

/// Result of a truncation operation
#[derive(Debug, Clone)]
pub struct TruncationResult {
    /// The (possibly truncated) content
    pub content: String,
    /// Original byte length
    pub original_bytes: usize,
    /// Whether truncation occurred
    pub truncated: bool,
    /// Number of bytes that were removed
    pub truncated_bytes: usize,
}

impl TruncationResult {
    /// Get the estimated token count of the result
    pub fn estimate_tokens(&self) -> u64 {
        approx_tokens_from_bytes(self.content.len())
    }
}

/// Truncate string to a maximum byte length, respecting UTF-8 boundaries
fn truncate_to_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }

    // Find the last valid UTF-8 boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    s[..end].to_string()
}

/// Model family specific limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLimits {
    /// Maximum context window size in tokens
    pub context_window: u64,
    /// Token threshold for auto-compaction
    pub auto_compact_threshold: u64,
    /// Default tool output byte limit
    pub tool_output_bytes: usize,
    /// Maximum output tokens per response
    pub max_output_tokens: u64,
}

impl ModelLimits {
    /// GPT-4 family limits
    pub fn gpt4() -> Self {
        Self {
            context_window: 128_000,
            auto_compact_threshold: 100_000,
            tool_output_bytes: 50_000,
            max_output_tokens: 16_384,
        }
    }

    /// GPT-4o limits
    pub fn gpt4o() -> Self {
        Self {
            context_window: 128_000,
            auto_compact_threshold: 100_000,
            tool_output_bytes: 50_000,
            max_output_tokens: 16_384,
        }
    }

    /// Claude 3 family limits
    pub fn claude3() -> Self {
        Self {
            context_window: 200_000,
            auto_compact_threshold: 150_000,
            tool_output_bytes: 50_000,
            max_output_tokens: 8_192,
        }
    }

    /// Claude 3.5 Sonnet limits
    pub fn claude35_sonnet() -> Self {
        Self {
            context_window: 200_000,
            auto_compact_threshold: 150_000,
            tool_output_bytes: 50_000,
            max_output_tokens: 8_192,
        }
    }

    /// Should trigger auto-compaction at this token count
    pub fn should_compact(&self, current_tokens: u64) -> bool {
        current_tokens >= self.auto_compact_threshold
    }

    /// Get truncation policy for tool output
    pub fn tool_output_policy(&self) -> TruncationPolicy {
        TruncationPolicy::Bytes(self.tool_output_bytes)
    }
}

impl Default for ModelLimits {
    fn default() -> Self {
        Self::gpt4o()
    }
}
