//! Shell types and configuration.
//!
//! This module provides shell type detection and configuration
//! for different shell environments.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Supported shell types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellType {
    /// Bash shell.
    Bash,
    /// Zsh shell.
    Zsh,
    /// POSIX sh shell.
    Sh,
    /// PowerShell (Windows/cross-platform).
    PowerShell,
    /// Windows Command Prompt.
    Cmd,
    /// Fish shell.
    Fish,
}

impl ShellType {
    /// Get the shell name.
    pub fn name(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Sh => "sh",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
            ShellType::Fish => "fish",
        }
    }

    /// Check if this shell uses POSIX-style arguments.
    pub fn is_posix(&self) -> bool {
        matches!(self, ShellType::Bash | ShellType::Zsh | ShellType::Sh | ShellType::Fish)
    }

    /// Get the command flag for executing a command string.
    pub fn command_flag(&self) -> &'static str {
        match self {
            ShellType::Bash | ShellType::Zsh | ShellType::Sh | ShellType::Fish => "-c",
            ShellType::PowerShell => "-Command",
            ShellType::Cmd => "/c",
        }
    }

    /// Get the login command flag (if supported).
    pub fn login_command_flag(&self) -> Option<&'static str> {
        match self {
            ShellType::Bash | ShellType::Zsh | ShellType::Sh => Some("-lc"),
            ShellType::Fish => Some("-l"),
            _ => None,
        }
    }
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Detect shell type from a path.
pub fn detect_shell_type(path: &Path) -> Option<ShellType> {
    let file_name = path.file_name()?.to_str()?.to_lowercase();

    // Strip .exe suffix on Windows
    let name = file_name.strip_suffix(".exe").unwrap_or(&file_name);

    match name {
        "bash" => Some(ShellType::Bash),
        "zsh" => Some(ShellType::Zsh),
        "sh" | "dash" | "ash" => Some(ShellType::Sh),
        "powershell" | "pwsh" => Some(ShellType::PowerShell),
        "cmd" => Some(ShellType::Cmd),
        "fish" => Some(ShellType::Fish),
        _ => None,
    }
}

/// Shell configuration with path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shell {
    /// Type of shell.
    pub shell_type: ShellType,

    /// Path to the shell executable.
    pub shell_path: PathBuf,
}

impl Shell {
    /// Create a new shell configuration.
    pub fn new(shell_type: ShellType, shell_path: PathBuf) -> Self {
        Self { shell_type, shell_path }
    }

    /// Get the shell name.
    pub fn name(&self) -> &'static str {
        self.shell_type.name()
    }

    /// Derive execution arguments for a command.
    ///
    /// Returns the full command line including shell path and arguments.
    pub fn derive_exec_args(&self, command: &str, use_login_shell: bool) -> Vec<String> {
        match self.shell_type {
            ShellType::Bash | ShellType::Zsh | ShellType::Sh => {
                let flag = if use_login_shell { "-lc" } else { "-c" };
                vec![
                    self.shell_path.to_string_lossy().into_owned(),
                    flag.to_string(),
                    command.to_string(),
                ]
            }
            ShellType::Fish => {
                let mut args = vec![self.shell_path.to_string_lossy().into_owned()];
                if use_login_shell {
                    args.push("-l".to_string());
                }
                args.push("-c".to_string());
                args.push(command.to_string());
                args
            }
            ShellType::PowerShell => {
                let mut args = vec![self.shell_path.to_string_lossy().into_owned()];
                if !use_login_shell {
                    args.push("-NoProfile".to_string());
                }
                args.push("-Command".to_string());
                args.push(command.to_string());
                args
            }
            ShellType::Cmd => {
                vec![
                    self.shell_path.to_string_lossy().into_owned(),
                    "/c".to_string(),
                    command.to_string(),
                ]
            }
        }
    }
}

/// Get the user's default shell.
#[cfg(unix)]
pub fn get_user_shell() -> Option<Shell> {
    // Try SHELL environment variable first (most reliable)
    if let Ok(shell_path) = std::env::var("SHELL") {
        let path = PathBuf::from(&shell_path);
        if let Some(shell_type) = detect_shell_type(&path) {
            if path.exists() {
                return Some(Shell::new(shell_type, path));
            }
        }
    }

    // Fall back to common shells
    for shell_type in [ShellType::Bash, ShellType::Zsh, ShellType::Sh] {
        if let Some(shell) = get_shell(shell_type) {
            return Some(shell);
        }
    }

    None
}

/// Get the user's default shell (Windows version).
#[cfg(not(unix))]
pub fn get_user_shell() -> Option<Shell> {
    // On Windows, default to PowerShell if available, otherwise cmd
    if let Ok(pwsh) = which::which("pwsh") {
        return Some(Shell::new(ShellType::PowerShell, pwsh));
    }

    if let Ok(powershell) = which::which("powershell") {
        return Some(Shell::new(ShellType::PowerShell, powershell));
    }

    // Fall back to cmd
    std::env::var("COMSPEC")
        .ok()
        .map(|path| Shell::new(ShellType::Cmd, PathBuf::from(path)))
}

/// Get a specific shell by type.
pub fn get_shell(shell_type: ShellType) -> Option<Shell> {
    let binary = shell_type.name();

    // Try to find the shell in PATH
    if let Ok(path) = which::which(binary) {
        return Some(Shell::new(shell_type, path));
    }

    // Try common locations
    let fallback_paths = match shell_type {
        ShellType::Bash => vec!["/bin/bash", "/usr/bin/bash"],
        ShellType::Zsh => vec!["/bin/zsh", "/usr/bin/zsh"],
        ShellType::Sh => vec!["/bin/sh", "/usr/bin/sh"],
        ShellType::Fish => vec!["/usr/bin/fish", "/usr/local/bin/fish"],
        ShellType::PowerShell => vec![],
        ShellType::Cmd => vec![],
    };

    for path in fallback_paths {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(Shell::new(shell_type, path));
        }
    }

    None
}

/// Get the default shell, with fallbacks.
pub fn default_shell() -> Shell {
    // Try user's shell first
    if let Some(shell) = get_user_shell() {
        return shell;
    }

    // Try common shells in order of preference
    for shell_type in [ShellType::Bash, ShellType::Zsh, ShellType::Sh] {
        if let Some(shell) = get_shell(shell_type) {
            return shell;
        }
    }

    // Ultimate fallback
    Shell::new(ShellType::Sh, PathBuf::from("/bin/sh"))
}
