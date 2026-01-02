//! Session services - shared infrastructure for a session.
//!
//! This module provides the shared services that are used across all turns
//! in a session, including MCP connections, execution policies, and shell management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use mms_exec::Executor;
use mms_execpolicy::ExecPolicyManager;
use mms_tools::ToolRegistry;

use crate::approval::{ApprovalStore, SharedApprovalManager};

/// Shell type enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Sh,
    Unknown(String),
}

impl ShellType {
    /// Detect shell type from path.
    pub fn from_path(path: &str) -> Self {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path);

        match name.to_lowercase().as_str() {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "fish" => ShellType::Fish,
            "pwsh" | "powershell" | "powershell.exe" => ShellType::PowerShell,
            "cmd" | "cmd.exe" => ShellType::Cmd,
            "sh" => ShellType::Sh,
            other => ShellType::Unknown(other.to_string()),
        }
    }

    /// Get the shell name.
    pub fn name(&self) -> &str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
            ShellType::Sh => "sh",
            ShellType::Unknown(s) => s,
        }
    }
}

/// Information about the user's shell.
#[derive(Debug, Clone)]
pub struct ShellInfo {
    /// Path to the shell executable.
    pub path: PathBuf,
    /// Type of shell.
    pub shell_type: ShellType,
    /// Environment variables for the shell.
    pub env: HashMap<String, String>,
    /// Shell-specific initialization commands.
    pub init_commands: Vec<String>,
}

impl Default for ShellInfo {
    fn default() -> Self {
        Self::detect()
    }
}

impl ShellInfo {
    /// Detect the user's default shell.
    pub fn detect() -> Self {
        #[cfg(unix)]
        let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        #[cfg(windows)]
        let shell_path = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());

        let shell_type = ShellType::from_path(&shell_path);
        let env = std::env::vars().collect();

        Self {
            path: PathBuf::from(shell_path),
            shell_type,
            env,
            init_commands: Vec::new(),
        }
    }

    /// Get command-line arguments for executing a command in this shell.
    pub fn command_args(&self) -> Vec<&str> {
        match self.shell_type {
            ShellType::Bash | ShellType::Zsh | ShellType::Sh => vec!["-c"],
            ShellType::Fish => vec!["-c"],
            ShellType::PowerShell => vec!["-Command"],
            ShellType::Cmd => vec!["/C"],
            ShellType::Unknown(_) => vec!["-c"],
        }
    }
}

/// MCP connection state for a single server.
#[derive(Debug, Clone)]
pub struct McpConnectionState {
    /// Server name/alias.
    pub name: String,
    /// Whether the server is connected.
    pub connected: bool,
    /// Available tools from this server.
    pub tools: Vec<String>,
    /// Error message if connection failed.
    pub error: Option<String>,
}

/// Manager for MCP server connections within a session.
#[derive(Debug, Default)]
pub struct McpSessionManager {
    /// Connection states for each server.
    connections: HashMap<String, McpConnectionState>,
    /// Startup cancellation token.
    startup_token: Option<CancellationToken>,
}

impl McpSessionManager {
    /// Create a new MCP session manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a connection state.
    pub fn register(&mut self, name: String, state: McpConnectionState) {
        self.connections.insert(name, state);
    }

    /// Get connection state for a server.
    pub fn get(&self, name: &str) -> Option<&McpConnectionState> {
        self.connections.get(name)
    }

    /// Get all connection states.
    pub fn all(&self) -> impl Iterator<Item = &McpConnectionState> {
        self.connections.values()
    }

    /// Set the startup cancellation token.
    pub fn set_startup_token(&mut self, token: CancellationToken) {
        self.startup_token = Some(token);
    }

    /// Cancel startup.
    pub fn cancel_startup(&self) {
        if let Some(token) = &self.startup_token {
            token.cancel();
        }
    }
}

/// User notification configuration.
#[derive(Debug, Clone, Default)]
pub struct NotificationConfig {
    /// Whether notifications are enabled.
    pub enabled: bool,
    /// Command to run for notifications.
    pub command: Option<String>,
}

/// User notifier for sending notifications.
#[derive(Debug)]
pub struct UserNotifier {
    config: NotificationConfig,
}

impl UserNotifier {
    /// Create a new user notifier.
    pub fn new(config: NotificationConfig) -> Self {
        Self { config }
    }

    /// Send a notification.
    pub async fn notify(&self, title: &str, message: &str) -> Result<(), std::io::Error> {
        if !self.config.enabled {
            return Ok(());
        }

        if let Some(cmd) = &self.config.command {
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd.replace("{title}", title).replace("{message}", message))
                .output()
                .await?;

            if !output.status.success() {
                tracing::warn!(
                    "Notification command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        Ok(())
    }
}

impl Default for UserNotifier {
    fn default() -> Self {
        Self::new(NotificationConfig::default())
    }
}

/// Shared services available to all turns in a session.
pub struct SessionServices {
    /// MCP connection manager.
    pub mcp_manager: Arc<RwLock<McpSessionManager>>,

    /// Cancellation token for MCP server startup.
    pub mcp_startup_token: CancellationToken,

    /// Tool execution executor.
    pub executor: Arc<Executor>,

    /// Tool registry.
    pub tool_registry: Arc<ToolRegistry>,

    /// User notifier.
    pub notifier: UserNotifier,

    /// User's shell information.
    pub user_shell: Arc<ShellInfo>,

    /// Whether to show raw agent reasoning.
    pub show_raw_reasoning: bool,

    /// Execution policy manager.
    pub exec_policy: Arc<ExecPolicyManager>,

    /// Approval store for tool executions.
    pub tool_approvals: Mutex<ApprovalStore>,

    /// Shared approval manager for pending approvals.
    pub approval_manager: SharedApprovalManager,
}

impl SessionServices {
    /// Create new session services.
    pub fn new(
        executor: Arc<Executor>,
        tool_registry: Arc<ToolRegistry>,
        exec_policy: Arc<ExecPolicyManager>,
        approval_manager: SharedApprovalManager,
    ) -> Self {
        Self {
            mcp_manager: Arc::new(RwLock::new(McpSessionManager::new())),
            mcp_startup_token: CancellationToken::new(),
            executor,
            tool_registry,
            notifier: UserNotifier::default(),
            user_shell: Arc::new(ShellInfo::detect()),
            show_raw_reasoning: false,
            exec_policy,
            tool_approvals: Mutex::new(ApprovalStore::default()),
            approval_manager,
        }
    }

    /// Get the user's shell.
    pub fn shell(&self) -> &ShellInfo {
        &self.user_shell
    }

    /// Check if a command is safe to execute.
    pub async fn is_command_safe(&self, command: &str) -> bool {
        self.exec_policy.is_allowed(command).await
    }
}
