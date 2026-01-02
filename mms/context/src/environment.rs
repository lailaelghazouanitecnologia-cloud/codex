//! Environment context for gathering runtime information.
//!
//! This module provides utilities for collecting information about
//! the execution environment including platform, shell, git, and user info.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Platform information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    /// Operating system (e.g., "linux", "macos", "windows").
    pub os: String,

    /// CPU architecture (e.g., "x86_64", "aarch64").
    pub arch: String,

    /// OS version string.
    pub os_version: Option<String>,

    /// Number of CPU cores.
    pub cpu_count: usize,

    /// Whether running in a container.
    pub in_container: bool,

    /// Whether running in WSL.
    pub in_wsl: bool,
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self::detect()
    }
}

impl PlatformInfo {
    /// Detect platform information from the current environment.
    pub fn detect() -> Self {
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();

        let os_version = Self::detect_os_version();
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        let in_container = Self::detect_container();
        let in_wsl = Self::detect_wsl();

        Self {
            os,
            arch,
            os_version,
            cpu_count,
            in_container,
            in_wsl,
        }
    }

    fn detect_os_version() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|content| {
                    for line in content.lines() {
                        if line.starts_with("PRETTY_NAME=") {
                            return line
                                .trim_start_matches("PRETTY_NAME=")
                                .trim_matches('"')
                                .to_string()
                                .into();
                        }
                    }
                    None
                })
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .and_then(|output| {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                })
        }

        #[cfg(target_os = "windows")]
        {
            std::env::var("OS").ok()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            None
        }
    }

    fn detect_container() -> bool {
        // Check for Docker
        if Path::new("/.dockerenv").exists() {
            return true;
        }

        // Check cgroup for container indicators
        if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
            if cgroup.contains("/docker/") || cgroup.contains("/lxc/") {
                return true;
            }
        }

        // Check for container environment variable
        std::env::var("container").is_ok()
    }

    fn detect_wsl() -> bool {
        if let Ok(version) = std::fs::read_to_string("/proc/version") {
            return version.to_lowercase().contains("microsoft")
                || version.to_lowercase().contains("wsl");
        }
        false
    }

    /// Get a display string for the platform.
    pub fn display_string(&self) -> String {
        let mut parts = vec![self.os.clone(), self.arch.clone()];

        if let Some(ref version) = self.os_version {
            parts.push(version.clone());
        }

        if self.in_wsl {
            parts.push("WSL".to_string());
        }

        if self.in_container {
            parts.push("container".to_string());
        }

        parts.join(" / ")
    }
}

/// Shell information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellInfo {
    /// Shell type (bash, zsh, fish, etc.).
    pub shell_type: String,

    /// Full path to the shell.
    pub shell_path: PathBuf,

    /// Shell version if available.
    pub version: Option<String>,

    /// Whether it's a login shell.
    pub is_login: bool,
}

impl Default for ShellInfo {
    fn default() -> Self {
        Self::detect()
    }
}

impl ShellInfo {
    /// Detect shell information from the environment.
    pub fn detect() -> Self {
        let shell_path = std::env::var("SHELL")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/bin/sh"));

        let shell_type = shell_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sh")
            .to_string();

        let version = Self::detect_version(&shell_path, &shell_type);

        Self {
            shell_type,
            shell_path,
            version,
            is_login: std::env::var("SHLVL")
                .ok()
                .and_then(|v| v.parse::<i32>().ok())
                .map(|lvl| lvl <= 1)
                .unwrap_or(false),
        }
    }

    fn detect_version(shell_path: &Path, shell_type: &str) -> Option<String> {
        let version_flag = match shell_type {
            "bash" | "zsh" | "fish" | "sh" => "--version",
            _ => return None,
        };

        std::process::Command::new(shell_path)
            .arg(version_flag)
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.lines().next().unwrap_or("").trim().to_string())
            })
    }
}

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Username.
    pub username: String,

    /// Home directory.
    pub home_dir: PathBuf,

    /// User ID (Unix only).
    pub uid: Option<u32>,

    /// Group ID (Unix only).
    pub gid: Option<u32>,

    /// Whether running as root/admin.
    pub is_admin: bool,
}

impl Default for UserInfo {
    fn default() -> Self {
        Self::detect()
    }
}

impl UserInfo {
    /// Detect user information from the environment.
    pub fn detect() -> Self {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

        #[cfg(unix)]
        let (uid, gid, is_admin) = {
            use std::os::unix::fs::MetadataExt;
            let uid = std::fs::metadata(&home_dir)
                .ok()
                .map(|m| m.uid());
            let gid = std::fs::metadata(&home_dir)
                .ok()
                .map(|m| m.gid());
            let is_admin = std::env::var("EUID")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .map(|euid| euid == 0)
                .unwrap_or(false);
            (uid, gid, is_admin)
        };

        #[cfg(not(unix))]
        let (uid, gid, is_admin) = (None, None, false);

        Self {
            username,
            home_dir,
            uid,
            gid,
            is_admin,
        }
    }
}

/// Git repository information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    /// Whether in a git repository.
    pub in_repo: bool,

    /// Repository root path.
    pub repo_root: Option<PathBuf>,

    /// Current branch name.
    pub branch: Option<String>,

    /// Current commit hash (short).
    pub commit_short: Option<String>,

    /// Whether there are uncommitted changes.
    pub has_changes: bool,

    /// Remote URL (origin).
    pub remote_url: Option<String>,
}

impl Default for GitInfo {
    fn default() -> Self {
        Self {
            in_repo: false,
            repo_root: None,
            branch: None,
            commit_short: None,
            has_changes: false,
            remote_url: None,
        }
    }
}

impl GitInfo {
    /// Detect git information for the given directory.
    pub fn detect(cwd: &Path) -> Self {
        // Check if in a git repo
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(cwd)
            .output();

        let in_repo = output
            .as_ref()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !in_repo {
            return Self::default();
        }

        // Get repo root
        let repo_root = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(cwd)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| PathBuf::from(s.trim()));

        // Get current branch
        let branch = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "HEAD");

        // Get short commit hash
        let commit_short = std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        // Check for changes
        let has_changes = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        // Get remote URL
        let remote_url = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        Self {
            in_repo,
            repo_root,
            branch,
            commit_short,
            has_changes,
            remote_url,
        }
    }
}

/// Available tool information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAvailability {
    /// Available command-line tools.
    pub tools: HashMap<String, ToolInfo>,
}

/// Information about a specific tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Tool name.
    pub name: String,

    /// Whether the tool is available.
    pub available: bool,

    /// Path to the tool.
    pub path: Option<PathBuf>,

    /// Version string.
    pub version: Option<String>,
}

impl ToolAvailability {
    /// Check availability of common development tools.
    pub fn detect() -> Self {
        let tools_to_check = [
            ("git", &["--version"][..]),
            ("node", &["--version"]),
            ("npm", &["--version"]),
            ("yarn", &["--version"]),
            ("pnpm", &["--version"]),
            ("python", &["--version"]),
            ("python3", &["--version"]),
            ("pip", &["--version"]),
            ("cargo", &["--version"]),
            ("rustc", &["--version"]),
            ("go", &["version"]),
            ("java", &["-version"]),
            ("docker", &["--version"]),
            ("kubectl", &["version", "--client", "--short"]),
            ("make", &["--version"]),
            ("cmake", &["--version"]),
            ("gcc", &["--version"]),
            ("clang", &["--version"]),
        ];

        let mut tools = HashMap::new();

        for (name, version_args) in tools_to_check {
            let info = Self::check_tool(name, version_args);
            tools.insert(name.to_string(), info);
        }

        Self { tools }
    }

    fn check_tool(name: &str, version_args: &[&str]) -> ToolInfo {
        // Check if tool exists using 'which'
        let which_result = std::process::Command::new("which")
            .arg(name)
            .output();

        let path = which_result
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| PathBuf::from(s.trim()));

        let available = path.is_some();

        let version = if available {
            std::process::Command::new(name)
                .args(version_args)
                .output()
                .ok()
                .and_then(|o| {
                    // Some tools output to stderr (like java)
                    let output = if o.stdout.is_empty() {
                        o.stderr
                    } else {
                        o.stdout
                    };
                    String::from_utf8(output).ok()
                })
                .map(|s| s.lines().next().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        ToolInfo {
            name: name.to_string(),
            available,
            path,
            version,
        }
    }

    /// Get a list of available tools.
    pub fn available_tools(&self) -> Vec<&str> {
        self.tools
            .iter()
            .filter(|(_, info)| info.available)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Check if a specific tool is available.
    pub fn is_available(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .map(|info| info.available)
            .unwrap_or(false)
    }
}

/// Complete environment context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentContext {
    /// Platform information.
    pub platform: PlatformInfo,

    /// Shell information.
    pub shell: ShellInfo,

    /// User information.
    pub user: UserInfo,

    /// Git repository information.
    pub git: GitInfo,

    /// Current working directory.
    pub cwd: PathBuf,

    /// Environment variables of interest.
    pub env_vars: HashMap<String, String>,

    /// Timestamp when context was captured.
    pub timestamp: String,
}

impl EnvironmentContext {
    /// Collect environment context for the given working directory.
    pub fn collect(cwd: &Path) -> Self {
        let platform = PlatformInfo::detect();
        let shell = ShellInfo::detect();
        let user = UserInfo::detect();
        let git = GitInfo::detect(cwd);

        // Collect interesting environment variables
        let env_keys = [
            "PATH", "HOME", "USER", "SHELL", "TERM", "LANG", "LC_ALL",
            "EDITOR", "VISUAL", "PAGER", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
            "NODE_ENV", "RUST_LOG", "DEBUG", "CI", "GITHUB_ACTIONS",
        ];

        let mut env_vars = HashMap::new();
        for key in env_keys {
            if let Ok(value) = std::env::var(key) {
                env_vars.insert(key.to_string(), value);
            }
        }

        let timestamp = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        Self {
            platform,
            shell,
            user,
            git,
            cwd: cwd.to_path_buf(),
            env_vars,
            timestamp,
        }
    }

    /// Generate a summary string suitable for LLM context.
    pub fn summary(&self) -> String {
        let mut parts = vec![
            format!("Platform: {}", self.platform.display_string()),
            format!("Shell: {}", self.shell.shell_type),
            format!("User: {}", self.user.username),
            format!("CWD: {}", self.cwd.display()),
        ];

        if self.git.in_repo {
            if let Some(ref branch) = self.git.branch {
                parts.push(format!("Git branch: {}", branch));
            }
            if self.git.has_changes {
                parts.push("Git: uncommitted changes".to_string());
            }
        }

        if self.env_vars.contains_key("CI") || self.env_vars.contains_key("GITHUB_ACTIONS") {
            parts.push("Environment: CI".to_string());
        }

        parts.join("\n")
    }

    /// Check if running in CI environment.
    pub fn is_ci(&self) -> bool {
        self.env_vars.contains_key("CI")
            || self.env_vars.contains_key("GITHUB_ACTIONS")
            || self.env_vars.contains_key("GITLAB_CI")
            || self.env_vars.contains_key("CIRCLECI")
    }

    /// Get the effective editor.
    pub fn editor(&self) -> Option<&str> {
        self.env_vars
            .get("VISUAL")
            .or_else(|| self.env_vars.get("EDITOR"))
            .map(|s| s.as_str())
    }
}

/// Builder for collecting specific parts of the environment context.
#[derive(Debug, Default)]
pub struct EnvironmentContextBuilder {
    include_platform: bool,
    include_shell: bool,
    include_user: bool,
    include_git: bool,
    include_env_vars: bool,
    include_tools: bool,
    cwd: Option<PathBuf>,
}

impl EnvironmentContextBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Include platform information.
    pub fn with_platform(mut self) -> Self {
        self.include_platform = true;
        self
    }

    /// Include shell information.
    pub fn with_shell(mut self) -> Self {
        self.include_shell = true;
        self
    }

    /// Include user information.
    pub fn with_user(mut self) -> Self {
        self.include_user = true;
        self
    }

    /// Include git information.
    pub fn with_git(mut self) -> Self {
        self.include_git = true;
        self
    }

    /// Include environment variables.
    pub fn with_env_vars(mut self) -> Self {
        self.include_env_vars = true;
        self
    }

    /// Include tool availability.
    pub fn with_tools(mut self) -> Self {
        self.include_tools = true;
        self
    }

    /// Include all context.
    pub fn all(mut self) -> Self {
        self.include_platform = true;
        self.include_shell = true;
        self.include_user = true;
        self.include_git = true;
        self.include_env_vars = true;
        self.include_tools = true;
        self
    }

    /// Set the working directory.
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Build and collect the environment context.
    pub fn build(self) -> EnvironmentContext {
        let cwd = self.cwd.unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });

        EnvironmentContext::collect(&cwd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detect() {
        let platform = PlatformInfo::detect();
        assert!(!platform.os.is_empty());
        assert!(!platform.arch.is_empty());
        assert!(platform.cpu_count >= 1);
    }

    #[test]
    fn test_shell_detect() {
        let shell = ShellInfo::detect();
        assert!(!shell.shell_type.is_empty());
    }

    #[test]
    fn test_user_detect() {
        let user = UserInfo::detect();
        assert!(!user.username.is_empty());
    }

    #[test]
    fn test_environment_context_collect() {
        let cwd = std::env::current_dir().unwrap();
        let ctx = EnvironmentContext::collect(&cwd);

        assert!(!ctx.platform.os.is_empty());
        assert!(!ctx.shell.shell_type.is_empty());
        assert_eq!(ctx.cwd, cwd);
    }
}
