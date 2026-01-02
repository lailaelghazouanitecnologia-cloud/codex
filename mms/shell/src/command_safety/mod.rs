//! Command safety analysis for shell command execution.
//!
//! This module provides comprehensive command safety analysis including:
//! - Safe command whitelisting with argument validation
//! - Dangerous command pattern detection
//! - Command classification for approval flow
//! - Shell chain analysis for compound commands
//!
//! # Architecture
//!
//! The safety system uses a multi-layer approach:
//!
//! 1. **Safe Command Detection** ([`is_known_safe_command`]):
//!    - Whitelist of known-safe commands (ls, cat, pwd, etc.)
//!    - Conditional safety based on arguments (find without -exec)
//!    - Shell chain validation (all commands in chain must be safe)
//!
//! 2. **Dangerous Command Detection** ([`is_dangerous_command`]):
//!    - Pattern-based detection of destructive commands
//!    - Recursive analysis for shell chains
//!    - Platform-specific patterns (Unix/Windows)
//!
//! 3. **Command Classification** ([`CommandClassification`]):
//!    - Combines safe/dangerous detection with policy
//!    - Provides approval requirements
//!    - Supports custom policy rules
//!
//! # Example
//!
//! ```rust
//! use mms_shell::command_safety::{
//!     is_known_safe_command, is_dangerous_command, classify_command,
//!     CommandClassification, ApprovalPolicy,
//! };
//!
//! // Check if a command is safe
//! assert!(is_known_safe_command(&["ls", "-la"]));
//! assert!(!is_known_safe_command(&["rm", "-rf", "/"]));
//!
//! // Check if a command is dangerous
//! assert!(is_dangerous_command(&["rm", "-rf", "/"]));
//! assert!(!is_dangerous_command(&["cat", "file.txt"]));
//!
//! // Classify a command for approval
//! let classification = classify_command(&["git", "push"], ApprovalPolicy::OnDanger);
//! match classification {
//!     CommandClassification::Safe => println!("No approval needed"),
//!     CommandClassification::NeedsApproval { reason, .. } => println!("Needs approval: {}", reason),
//!     CommandClassification::Forbidden { reason } => println!("Forbidden: {}", reason),
//! }
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod classifier;
mod dangerous_commands;
mod safe_commands;

pub use classifier::{
    ApprovalPolicy, CommandClassification, CommandRisk, RiskFactor,
    classify_command, classify_command_with_context,
};
pub use dangerous_commands::{
    is_dangerous_command, is_dangerous_to_exec, DangerousPattern,
    get_danger_reason, check_dangerous_patterns,
};
pub use safe_commands::{
    is_known_safe_command, is_safe_to_exec, SafeCommandResult,
    check_conditional_safety, get_base_safe_commands,
};

/// Re-export commonly used types
pub mod prelude {
    pub use super::{
        ApprovalPolicy, CommandClassification, CommandRisk,
        classify_command, is_dangerous_command, is_known_safe_command,
    };
}
