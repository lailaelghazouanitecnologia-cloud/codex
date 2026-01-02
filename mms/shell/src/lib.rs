//! Shell command parsing and execution for MMS agent.
//!
//! This crate provides comprehensive shell support including:
//! - Shell type detection (bash, zsh, sh, PowerShell, etc.)
//! - Command parsing and classification
//! - Safe command execution with timeout support
//! - Cross-platform shell detection
//!
//! # Example
//!
//! ```ignore
//! use mms_shell::{parse_command, Shell, ShellType, exec};
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
//! // Execute a command
//! let output = exec("ls -la").await?;
//! println!("{}", output.stdout);
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

pub mod exec;
pub mod parse;
pub mod types;

// Re-exports
pub use exec::{
    CommandBuilder, ExecConfig, ExecError, ExecOutput, ExecResult,
    exec, exec_blocking, exec_expect_success, exec_with_config,
    is_safe_command, shell_escape,
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
