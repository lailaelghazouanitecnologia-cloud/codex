//! Rich response item types for conversation history.
//!
//! These types mirror the Codex CLI ResponseItem variants but adapted for MMS.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single item in the conversation history.
///
/// This is richer than a simple Message and supports:
/// - Function calls and their outputs
/// - Reasoning/thinking blocks
/// - Compaction summaries
/// - Ghost snapshots for uncommitted state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    /// A text message from user or assistant
    Message(MessageItem),

    /// A function/tool call from the assistant
    FunctionCall(FunctionCallItem),

    /// The output/result of a function call
    FunctionOutput(FunctionOutputItem),

    /// Reasoning/thinking content (may be encrypted)
    Reasoning(ReasoningItem),

    /// A compacted summary of previous turns
    Compaction(CompactionItem),

    /// A ghost snapshot of uncommitted file changes
    GhostSnapshot(GhostSnapshotItem),

    /// System message or instruction
    System(SystemItem),
}

/// A text message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageItem {
    pub role: MessageRole,
    pub content: String,
    /// Optional name for multi-agent scenarios
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// A function/tool call made by the assistant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallItem {
    /// Unique ID for this call (used to match with output)
    pub call_id: String,
    /// Name of the function/tool
    pub name: String,
    /// JSON arguments
    pub arguments: Value,
    /// Whether this call requires approval
    #[serde(default)]
    pub requires_approval: bool,
    /// Approval status if requires_approval is true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_status: Option<ApprovalStatus>,
}

/// Status of an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    TimedOut,
}

/// The output/result of a function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionOutputItem {
    /// The call_id this output corresponds to
    pub call_id: String,
    /// The output content (may be truncated)
    pub content: String,
    /// Whether this output represents an error
    #[serde(default)]
    pub is_error: bool,
    /// Original byte length before truncation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_bytes: Option<usize>,
    /// Whether this output was truncated
    #[serde(default)]
    pub truncated: bool,
}

/// Reasoning/thinking content from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningItem {
    /// The reasoning content (may be encrypted for some providers)
    pub content: String,
    /// Whether the content is encrypted
    #[serde(default)]
    pub encrypted: bool,
    /// Encrypted content if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
}

/// A compacted summary of previous conversation turns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionItem {
    /// Summary of the compacted content
    pub summary: String,
    /// Number of turns that were compacted
    pub turns_compacted: usize,
    /// Approximate token count of original content
    pub original_tokens: u64,
    /// Encrypted content if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
}

/// A ghost snapshot of uncommitted file changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostSnapshotItem {
    /// Path to the file
    pub path: String,
    /// The uncommitted content
    pub content: String,
    /// Whether this is a new file
    #[serde(default)]
    pub is_new: bool,
}

/// A system message or instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemItem {
    pub content: String,
    /// Whether this is a developer instruction
    #[serde(default)]
    pub is_developer: bool,
}

impl ResponseItem {
    /// Create a user message
    pub fn user_message(content: impl Into<String>) -> Self {
        Self::Message(MessageItem {
            role: MessageRole::User,
            content: content.into(),
            name: None,
        })
    }

    /// Create an assistant message
    pub fn assistant_message(content: impl Into<String>) -> Self {
        Self::Message(MessageItem {
            role: MessageRole::Assistant,
            content: content.into(),
            name: None,
        })
    }

    /// Create a function call
    pub fn function_call(call_id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self::FunctionCall(FunctionCallItem {
            call_id: call_id.into(),
            name: name.into(),
            arguments,
            requires_approval: false,
            approval_status: None,
        })
    }

    /// Create a function output
    pub fn function_output(call_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self {
        Self::FunctionOutput(FunctionOutputItem {
            call_id: call_id.into(),
            content: content.into(),
            is_error,
            original_bytes: None,
            truncated: false,
        })
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::System(SystemItem {
            content: content.into(),
            is_developer: false,
        })
    }

    /// Estimate the token count for this item
    pub fn estimate_tokens(&self) -> u64 {
        let content = match self {
            Self::Message(m) => &m.content,
            Self::FunctionCall(f) => {
                // Estimate based on name + serialized arguments
                let args_str = serde_json::to_string(&f.arguments).unwrap_or_default();
                return super::token::estimate_tokens(&f.name) + super::token::estimate_tokens(&args_str);
            }
            Self::FunctionOutput(o) => &o.content,
            Self::Reasoning(r) => {
                if let Some(encrypted) = &r.encrypted_content {
                    return super::token::estimate_reasoning_tokens(encrypted.len());
                }
                &r.content
            }
            Self::Compaction(c) => {
                if let Some(encrypted) = &c.encrypted_content {
                    return super::token::estimate_reasoning_tokens(encrypted.len());
                }
                &c.summary
            }
            Self::GhostSnapshot(g) => &g.content,
            Self::System(s) => &s.content,
        };
        super::token::estimate_tokens(content)
    }

    /// Get the role if this is a message
    pub fn role(&self) -> Option<MessageRole> {
        match self {
            Self::Message(m) => Some(m.role),
            Self::FunctionCall(_) => Some(MessageRole::Assistant),
            Self::FunctionOutput(_) => Some(MessageRole::Tool),
            Self::System(_) => Some(MessageRole::System),
            _ => None,
        }
    }

    /// Check if this item corresponds to a function call
    pub fn is_function_call(&self) -> bool {
        matches!(self, Self::FunctionCall(_))
    }

    /// Check if this item is a function output
    pub fn is_function_output(&self) -> bool {
        matches!(self, Self::FunctionOutput(_))
    }

    /// Get the call_id if this is a function call or output
    pub fn call_id(&self) -> Option<&str> {
        match self {
            Self::FunctionCall(f) => Some(&f.call_id),
            Self::FunctionOutput(o) => Some(&o.call_id),
            _ => None,
        }
    }
}

impl FunctionOutputItem {
    /// Create a truncated output
    pub fn truncated(call_id: impl Into<String>, content: String, original_bytes: usize) -> Self {
        Self {
            call_id: call_id.into(),
            content,
            is_error: false,
            original_bytes: Some(original_bytes),
            truncated: true,
        }
    }
}
