//! Git repository information and operations for MMS agent.
//!
//! This crate provides functionality for:
//! - Git repository detection
//! - Collecting git information (commit, branch, remote)
//! - Git diff tracking
//! - Branch management
//!
//! # Example
//!
//! ```ignore
//! use mms_git::{collect_git_info, get_git_repo_root};
//! use std::path::Path;
//!
//! let cwd = Path::new("/path/to/project");
//!
//! // Check if we're in a git repo
//! if let Some(root) = get_git_repo_root(cwd) {
//!     println!("Git root: {}", root.display());
//!
//!     // Get git info
//!     if let Some(info) = collect_git_info(cwd).await {
//!         println!("Branch: {:?}", info.branch);
//!         println!("Commit: {:?}", info.commit_hash);
//!     }
//! }
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Command;
use tracing::debug;

/// Default timeout for git commands.
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors that can occur during git operations.
#[derive(Debug, Error)]
pub enum GitError {
    /// IO error during git operation.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Git command failed.
    #[error("Git command failed: {0}")]
    CommandFailed(String),

    /// Git command timed out.
    #[error("Git command timed out after {0:?}")]
    Timeout(Duration),

    /// Not in a git repository.
    #[error("Not in a git repository")]
    NotARepo,
}

/// Result type for git operations.
pub type GitResult<T> = Result<T, GitError>;

/// Git repository information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitInfo {
    /// Current commit hash (SHA).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,

    /// Current branch name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,

    /// Remote repository URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,

    /// Whether the working tree has changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_changes: Option<bool>,

    /// Number of commits ahead of remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ahead: Option<usize>,

    /// Number of commits behind remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<usize>,
}

/// A commit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitLogEntry {
    /// Commit SHA.
    pub sha: String,

    /// Unix timestamp (seconds since epoch) of commit time.
    pub timestamp: i64,

    /// Single-line subject of commit message.
    pub subject: String,
}

/// Git diff to remote tracking branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffToRemote {
    /// SHA of the closest remote commit.
    pub sha: String,

    /// Unified diff text.
    pub diff: String,
}

/// Check if a path is inside a git repository.
///
/// This walks up the directory hierarchy looking for a `.git` file or directory.
/// Does not require the git binary.
pub fn is_git_repo(path: &Path) -> bool {
    get_git_repo_root(path).is_some()
}

/// Get the root of a git repository.
///
/// Walks up the directory hierarchy looking for a `.git` file or directory.
/// Returns None if not in a git repository.
pub fn get_git_repo_root(base_dir: &Path) -> Option<PathBuf> {
    let mut dir = if base_dir.is_absolute() {
        base_dir.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(base_dir)
    };

    loop {
        let git_path = dir.join(".git");
        if git_path.exists() {
            return Some(dir);
        }

        if !dir.pop() {
            break;
        }
    }

    None
}

/// Resolve git repository root for trust/config resolution.
///
/// This handles git worktrees by using `git rev-parse --git-common-dir`.
pub async fn resolve_git_root_for_trust(cwd: &Path) -> Option<PathBuf> {
    let output = run_git_command_with_timeout(&["rev-parse", "--git-common-dir"], cwd).await?;

    if !output.status.success() {
        return get_git_repo_root(cwd);
    }

    let git_common_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // If it's a worktree, --git-common-dir returns the path to the shared .git
    let git_path = PathBuf::from(&git_common_dir);
    if git_path.is_absolute() {
        // Go up one level from .git to get repo root
        git_path.parent().map(|p| p.to_path_buf())
    } else {
        // Relative path, resolve from cwd
        let resolved = cwd.join(&git_common_dir);
        resolved.parent().map(|p| p.to_path_buf())
    }
}

/// Collect git repository information.
///
/// Gathers commit hash, branch name, and remote URL in parallel.
/// Returns None if not in a git repository.
pub async fn collect_git_info(cwd: &Path) -> Option<GitInfo> {
    // Check if we're in a git repo first
    let check = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd).await?;
    if !check.status.success() {
        return None;
    }

    // Run all git commands in parallel
    let (commit, branch, url, status) = tokio::join!(
        run_git_command_with_timeout(&["rev-parse", "HEAD"], cwd),
        run_git_command_with_timeout(&["rev-parse", "--abbrev-ref", "HEAD"], cwd),
        run_git_command_with_timeout(&["remote", "get-url", "origin"], cwd),
        run_git_command_with_timeout(&["status", "--porcelain"], cwd)
    );

    let mut info = GitInfo::default();

    // Process commit hash
    if let Some(output) = commit {
        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !hash.is_empty() {
                info.commit_hash = Some(hash);
            }
        }
    }

    // Process branch name
    if let Some(output) = branch {
        if output.status.success() {
            let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch_name.is_empty() && branch_name != "HEAD" {
                info.branch = Some(branch_name);
            }
        }
    }

    // Process repository URL
    if let Some(output) = url {
        if output.status.success() {
            let repo_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !repo_url.is_empty() {
                info.repository_url = Some(repo_url);
            }
        }
    }

    // Process status
    if let Some(output) = status {
        if output.status.success() {
            let status_text = String::from_utf8_lossy(&output.stdout);
            info.has_changes = Some(!status_text.trim().is_empty());
        }
    }

    Some(info)
}

/// Get the current branch name.
pub async fn current_branch(cwd: &Path) -> Option<String> {
    let output = run_git_command_with_timeout(&["branch", "--show-current"], cwd).await?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

/// Get all local branch names.
pub async fn local_branches(cwd: &Path) -> Vec<String> {
    let output = run_git_command_with_timeout(
        &["branch", "--format=%(refname:short)"],
        cwd,
    )
    .await;

    let Some(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Get the default branch name (main, master, etc.).
pub async fn default_branch(cwd: &Path) -> Option<String> {
    // Try to get from remote HEAD symbolic ref
    let remotes = get_remotes(cwd).await;
    let remote = remotes.first().cloned().unwrap_or_else(|| "origin".to_string());

    // Try symbolic-ref first
    let sym_ref = format!("refs/remotes/{}/HEAD", remote);
    let output = run_git_command_with_timeout(
        &["symbolic-ref", "--short", &sym_ref],
        cwd,
    )
    .await;

    if let Some(output) = output {
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Extract just the branch name from "origin/main"
            if let Some(name) = branch.split('/').last() {
                return Some(name.to_string());
            }
        }
    }

    // Try git remote show
    let output = run_git_command_with_timeout(&["remote", "show", &remote], cwd).await;

    if let Some(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("HEAD branch:") {
                    if let Some(branch) = line.split(':').nth(1) {
                        let branch = branch.trim();
                        if !branch.is_empty() && branch != "(unknown)" {
                            return Some(branch.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fall back to checking if main or master exists locally
    let branches = local_branches(cwd).await;
    for default in &["main", "master"] {
        if branches.iter().any(|b| b == *default) {
            return Some((*default).to_string());
        }
    }

    None
}

/// Get recent commits.
pub async fn recent_commits(cwd: &Path, limit: usize) -> Vec<CommitLogEntry> {
    // Check if we're in a git repo
    let check = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd).await;
    if check.map(|o| o.status.success()) != Some(true) {
        return Vec::new();
    }

    let format = "%H%x1f%ct%x1f%s"; // SHA<US>timestamp<US>subject
    let limit_str = limit.to_string();

    let output = run_git_command_with_timeout(
        &["log", "-n", &limit_str, &format!("--pretty=format:{}", format)],
        cwd,
    )
    .await;

    let Some(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split('\x1f').collect();
        if parts.len() >= 3 {
            let sha = parts[0].to_string();
            let timestamp = parts[1].parse::<i64>().unwrap_or(0);
            let subject = parts[2].to_string();

            entries.push(CommitLogEntry {
                sha,
                timestamp,
                subject,
            });
        }
    }

    entries
}

/// Get git diff relative to remote tracking branch.
pub async fn diff_to_remote(cwd: &Path) -> Option<GitDiffToRemote> {
    // Get current branch
    let branch = current_branch(cwd).await?;

    // Get tracking branch
    let tracking_output = run_git_command_with_timeout(
        &["rev-parse", "--abbrev-ref", &format!("{}@{{upstream}}", branch)],
        cwd,
    )
    .await?;

    if !tracking_output.status.success() {
        return None;
    }

    let tracking = String::from_utf8_lossy(&tracking_output.stdout)
        .trim()
        .to_string();

    // Get merge-base (closest common ancestor)
    let base_output = run_git_command_with_timeout(
        &["merge-base", &tracking, "HEAD"],
        cwd,
    )
    .await?;

    if !base_output.status.success() {
        return None;
    }

    let base_sha = String::from_utf8_lossy(&base_output.stdout)
        .trim()
        .to_string();

    // Get diff
    let diff_output = run_git_command_with_timeout(
        &["diff", "--no-textconv", "--no-ext-diff", &base_sha],
        cwd,
    )
    .await?;

    if !diff_output.status.success() {
        return None;
    }

    let diff = String::from_utf8_lossy(&diff_output.stdout).into_owned();

    Some(GitDiffToRemote {
        sha: base_sha,
        diff,
    })
}

/// Get git remotes.
async fn get_remotes(cwd: &Path) -> Vec<String> {
    let output = run_git_command_with_timeout(&["remote"], cwd).await;

    let Some(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let mut remotes: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Prioritize "origin"
    if let Some(pos) = remotes.iter().position(|r| r == "origin") {
        let origin = remotes.remove(pos);
        remotes.insert(0, origin);
    }

    remotes
}

/// Run a git command with timeout.
async fn run_git_command_with_timeout(args: &[&str], cwd: &Path) -> Option<Output> {
    debug!("Running git command: git {:?} in {:?}", args, cwd);

    let result = tokio::time::timeout(
        GIT_COMMAND_TIMEOUT,
        Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Some(output),
        Ok(Err(e)) => {
            debug!("Git command failed: {}", e);
            None
        }
        Err(_) => {
            debug!("Git command timed out");
            None
        }
    }
}

/// Run a git command synchronously with timeout (blocking).
pub fn run_git_command_blocking(args: &[&str], cwd: &Path) -> Option<Output> {
    use std::process::Command;

    let result = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output();

    match result {
        Ok(output) => Some(output),
        Err(e) => {
            debug!("Git command failed: {}", e);
            None
        }
    }
}

/// Compute the git blob SHA for a file.
pub fn git_blob_oid(path: &Path, cwd: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    let output = run_git_command_blocking(&["hash-object", &path_str], cwd)?;

    if !output.status.success() {
        return None;
    }

    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

/// Get path relative to git root.
pub fn relative_to_git_root(path: &Path, git_root: &Path) -> Option<PathBuf> {
    path.strip_prefix(git_root).ok().map(|p| p.to_path_buf())
}
