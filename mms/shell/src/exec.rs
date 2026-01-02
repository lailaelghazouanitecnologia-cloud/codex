//! Shell command execution utilities.
//!
//! This module provides utilities for executing shell commands
//! with proper environment setup and output handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{Shell, default_shell};

/// Errors that can occur during command execution.
#[derive(Debug, Error)]
pub enum ExecError {
    /// IO error during execution.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Command timed out.
    #[error("command timed out after {0:?}")]
    Timeout(Duration),

    /// Command was cancelled.
    #[error("command was cancelled")]
    Cancelled,

    /// Command execution failed.
    #[error("command failed with exit code {0}")]
    ExitCode(i32),
}

/// Result type for execution operations.
pub type ExecResult<T> = Result<T, ExecError>;

/// Configuration for command execution.
#[derive(Debug, Clone)]
pub struct ExecConfig {
    /// Working directory for the command.
    pub cwd: Option<PathBuf>,

    /// Environment variables to set.
    pub env: HashMap<String, String>,

    /// Whether to use a login shell.
    pub use_login_shell: bool,

    /// Timeout for the command.
    pub timeout: Option<Duration>,

    /// Maximum output size in bytes.
    pub max_output_size: Option<usize>,

    /// Shell to use (defaults to user's shell).
    pub shell: Option<Shell>,
}

impl Default for ExecConfig {
    fn default() -> Self {
        Self {
            cwd: None,
            env: HashMap::new(),
            use_login_shell: true,
            timeout: Some(Duration::from_secs(120)),
            max_output_size: Some(100 * 1024), // 100KB
            shell: None,
        }
    }
}

impl ExecConfig {
    /// Create a new execution config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the working directory.
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set whether to use a login shell.
    pub fn with_login_shell(mut self, use_login: bool) -> Self {
        self.use_login_shell = use_login;
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set no timeout.
    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Set the shell to use.
    pub fn with_shell(mut self, shell: Shell) -> Self {
        self.shell = Some(shell);
        self
    }
}

/// Result of command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecOutput {
    /// Standard output.
    pub stdout: String,

    /// Standard error.
    pub stderr: String,

    /// Exit code (None if killed by signal).
    pub exit_code: Option<i32>,

    /// Duration of execution.
    pub duration: Duration,

    /// Whether output was truncated.
    pub truncated: bool,
}

impl ExecOutput {
    /// Check if the command succeeded (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Get combined output (stdout + stderr).
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

/// Execute a command with the default shell.
pub async fn exec(command: &str) -> ExecResult<ExecOutput> {
    exec_with_config(command, ExecConfig::default()).await
}

/// Execute a command with custom configuration.
pub async fn exec_with_config(command: &str, config: ExecConfig) -> ExecResult<ExecOutput> {
    let shell = config.shell.clone().unwrap_or_else(default_shell);
    let args = shell.derive_exec_args(command, config.use_login_shell);

    if args.is_empty() {
        return Err(ExecError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty command",
        )));
    }

    let program = &args[0];
    let program_args = &args[1..];

    let mut cmd = tokio::process::Command::new(program);
    cmd.args(program_args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());

    // Set working directory
    if let Some(ref cwd) = config.cwd {
        cmd.current_dir(cwd);
    }

    // Set environment variables
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    let start = Instant::now();

    // Spawn the process
    let child = cmd.spawn()?;

    // Handle timeout using select
    let output = if let Some(timeout) = config.timeout {
        tokio::select! {
            result = child.wait_with_output() => {
                result?
            }
            _ = tokio::time::sleep(timeout) => {
                // Timeout elapsed - the child will be killed when dropped
                return Err(ExecError::Timeout(timeout));
            }
        }
    } else {
        child.wait_with_output().await?
    };

    let duration = start.elapsed();

    // Convert output to strings, handling potential non-UTF8
    let max_size = config.max_output_size.unwrap_or(usize::MAX);
    let (stdout, stdout_truncated) = truncate_output(&output.stdout, max_size);
    let (stderr, stderr_truncated) = truncate_output(&output.stderr, max_size);

    Ok(ExecOutput {
        stdout,
        stderr,
        exit_code: output.status.code(),
        duration,
        truncated: stdout_truncated || stderr_truncated,
    })
}

/// Execute a command and return the output, failing if exit code is non-zero.
pub async fn exec_expect_success(command: &str) -> ExecResult<ExecOutput> {
    let output = exec(command).await?;
    if !output.success() {
        return Err(ExecError::ExitCode(output.exit_code.unwrap_or(-1)));
    }
    Ok(output)
}

/// Execute a command synchronously (blocking).
pub fn exec_blocking(command: &str, config: ExecConfig) -> ExecResult<ExecOutput> {
    let shell = config.shell.clone().unwrap_or_else(default_shell);
    let args = shell.derive_exec_args(command, config.use_login_shell);

    if args.is_empty() {
        return Err(ExecError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "empty command",
        )));
    }

    let program = &args[0];
    let program_args = &args[1..];

    let mut cmd = std::process::Command::new(program);
    cmd.args(program_args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());

    if let Some(ref cwd) = config.cwd {
        cmd.current_dir(cwd);
    }

    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    let start = Instant::now();
    let output = cmd.output()?;
    let duration = start.elapsed();

    let max_size = config.max_output_size.unwrap_or(usize::MAX);
    let (stdout, stdout_truncated) = truncate_output(&output.stdout, max_size);
    let (stderr, stderr_truncated) = truncate_output(&output.stderr, max_size);

    Ok(ExecOutput {
        stdout,
        stderr,
        exit_code: output.status.code(),
        duration,
        truncated: stdout_truncated || stderr_truncated,
    })
}

/// Truncate output to maximum size.
fn truncate_output(bytes: &[u8], max_size: usize) -> (String, bool) {
    let truncated = bytes.len() > max_size;
    let bytes = if truncated { &bytes[..max_size] } else { bytes };

    let text = String::from_utf8_lossy(bytes).into_owned();
    (text, truncated)
}

/// Builder for constructing complex shell commands.
#[derive(Debug, Default)]
pub struct CommandBuilder {
    parts: Vec<String>,
    connectors: Vec<String>,
}

impl CommandBuilder {
    /// Create a new command builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a simple command.
    pub fn cmd(mut self, command: &str) -> Self {
        self.parts.push(command.to_string());
        self
    }

    /// Add a command with arguments.
    pub fn cmd_with_args(mut self, program: &str, args: &[&str]) -> Self {
        let full = crate::parse::shell_join(
            &std::iter::once(program.to_string())
                .chain(args.iter().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
        );
        self.parts.push(full);
        self
    }

    /// Connect with AND (&&).
    pub fn and(mut self) -> Self {
        self.connectors.push("&&".to_string());
        self
    }

    /// Connect with OR (||).
    pub fn or(mut self) -> Self {
        self.connectors.push("||".to_string());
        self
    }

    /// Connect with PIPE (|).
    pub fn pipe(mut self) -> Self {
        self.connectors.push("|".to_string());
        self
    }

    /// Connect with SEQUENCE (;).
    pub fn seq(mut self) -> Self {
        self.connectors.push(";".to_string());
        self
    }

    /// Build the final command string.
    pub fn build(self) -> String {
        if self.parts.is_empty() {
            return String::new();
        }

        let mut result = self.parts[0].clone();
        for (i, part) in self.parts.iter().skip(1).enumerate() {
            let connector = self.connectors.get(i).map(|s| s.as_str()).unwrap_or("&&");
            result.push_str(&format!(" {} {}", connector, part));
        }
        result
    }
}

/// Escape a string for safe use in shell commands.
pub fn shell_escape(s: &str) -> String {
    shell_words::quote(s).into_owned()
}

/// Check if a command is safe (no dangerous characters).
pub fn is_safe_command(command: &str) -> bool {
    // Check for potentially dangerous patterns
    let dangerous_patterns = [
        "rm -rf /",
        "rm -rf ~",
        "rm -rf /*",
        "mkfs",
        "dd if=",
        "> /dev/sd",
        ":(){ :|:& };:",
        "chmod -R 777 /",
        "chown -R",
    ];

    for pattern in &dangerous_patterns {
        if command.contains(pattern) {
            return false;
        }
    }

    true
}
