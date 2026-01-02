//! Shell command parsing and execution for MMS agent.
//!
//! This crate provides comprehensive shell support including:
//! - Shell type detection (bash, zsh, sh, PowerShell, etc.)
//! - Command parsing and classification
//! - Safe command execution with timeout support
//! - Cross-platform shell detection
//! - Command safety analysis (safe/dangerous detection)
//!
//! # Example
//!
//! ```ignore
//! use mms_shell::{parse_command, Shell, ShellType, exec};
//! use mms_shell::command_safety::{is_known_safe_command, classify_command, ApprovalPolicy};
//!
//! // Parse a shell command
//! let commands = parse_command(&vec![
//!     "bash".to_string(),
//!     "-lc".to_string(),
//!     "cat file.txt && grep pattern".to_string(),
//! ]);
//!
//! for cmd in commands {
//!     println!("{}", cmd.description());
//! }
//!
//! // Check if command is safe
//! assert!(is_known_safe_command(&["ls", "-la"]));
//! assert!(!is_known_safe_command(&["rm", "-rf", "/"]));
//!
//! // Classify command for approval
//! let classification = classify_command(&["git", "push"], ApprovalPolicy::OnDanger);
//!
//! // Execute a command
//! let output = exec("ls -la").await?;
//! println!("{}", output.stdout);
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

pub mod command_safety;
pub mod exec;
pub mod parse;
pub mod types;

// Re-exports
pub use exec::{
    CommandBuilder, ExecConfig, ExecError, ExecOutput, ExecResult,
    exec, exec_blocking, exec_expect_success, exec_with_config,
    shell_escape,
};
pub use parse::{
    ParseError, ParseResult, ParsedCommand,
    extract_bash_command, extract_powershell_command, extract_shell_command,
    parse_command, shell_join, shell_split,
};
pub use types::{
    Shell, ShellType,
    default_shell, detect_shell_type, get_shell, get_user_shell,
};

// Re-export command safety for convenience
pub use command_safety::{
    is_known_safe_command, is_dangerous_command, classify_command,
    ApprovalPolicy, CommandClassification, CommandRisk,
};
