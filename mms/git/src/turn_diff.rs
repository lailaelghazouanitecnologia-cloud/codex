//! Turn diff tracking for undo functionality.
//!
//! This module tracks file changes during a conversation turn,
//! enabling undo operations and change visualization.
//!
//! # Overview
//!
//! The [`TurnDiffTracker`] monitors file operations during a turn:
//! - File creations
//! - File modifications
//! - File deletions
//! - File renames
//!
//! After a turn completes, the accumulated changes can be:
//! - Reported as a [`TurnDiff`] event
//! - Reverted using the [`revert`] method
//! - Inspected for visualization
//!
//! # Example
//!
//! ```ignore
//! use mms_git::turn_diff::{TurnDiffTracker, FileChange};
//!
//! // Create a new tracker for a turn
//! let mut tracker = TurnDiffTracker::new("turn_123".to_string());
//!
//! // Record file changes
//! tracker.record_creation("/path/to/new_file.rs");
//! tracker.record_modification("/path/to/existing.rs", Some(original_content));
//!
//! // Get the diff
//! let diff = tracker.finish();
//! println!("Turn modified {} files", diff.changes.len());
//!
//! // Optionally revert changes
//! tracker.revert().await?;
//! ```

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::GitResult;

// ============================================================================
// File Mode Tracking
// ============================================================================

/// Unix file mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileMode(pub u32);

impl FileMode {
    /// Regular file (644).
    pub const REGULAR: FileMode = FileMode(0o100644);

    /// Executable file (755).
    pub const EXECUTABLE: FileMode = FileMode(0o100755);

    /// Symbolic link.
    pub const SYMLINK: FileMode = FileMode(0o120000);

    /// Directory.
    pub const DIRECTORY: FileMode = FileMode(0o040000);

    /// Check if this mode represents a regular file.
    pub fn is_regular(&self) -> bool {
        (self.0 & 0o170000) == 0o100000
    }

    /// Check if this mode represents an executable.
    pub fn is_executable(&self) -> bool {
        self.0 == Self::EXECUTABLE.0
    }

    /// Check if this mode represents a symlink.
    pub fn is_symlink(&self) -> bool {
        (self.0 & 0o170000) == 0o120000
    }

    /// Check if this mode represents a directory.
    pub fn is_directory(&self) -> bool {
        (self.0 & 0o170000) == 0o040000
    }

    /// Get file mode from filesystem.
    #[cfg(unix)]
    pub fn from_path(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;

        std::fs::symlink_metadata(path).ok().map(|m| {
            let mode = m.mode();
            // Normalize to git-style mode
            if m.file_type().is_symlink() {
                Self::SYMLINK
            } else if m.is_dir() {
                Self::DIRECTORY
            } else if mode & 0o111 != 0 {
                Self::EXECUTABLE
            } else {
                Self::REGULAR
            }
        })
    }

    /// Get file mode from filesystem (non-Unix always returns REGULAR).
    #[cfg(not(unix))]
    pub fn from_path(path: &Path) -> Option<Self> {
        std::fs::symlink_metadata(path).ok().map(|m| {
            if m.file_type().is_symlink() {
                Self::SYMLINK
            } else if m.is_dir() {
                Self::DIRECTORY
            } else {
                Self::REGULAR
            }
        })
    }

    /// Format as git-style mode string.
    pub fn as_git_mode(&self) -> &'static str {
        match self.0 {
            0o100644 => "100644",
            0o100755 => "100755",
            0o120000 => "120000",
            0o040000 => "040000",
            _ => "100644",
        }
    }
}

impl Default for FileMode {
    fn default() -> Self {
        Self::REGULAR
    }
}

impl std::fmt::Display for FileMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:06o}", self.0)
    }
}

// ============================================================================
// Object ID (OID) Computation
// ============================================================================

/// Object ID for content hashing (similar to git blob SHA).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

impl ObjectId {
    /// Null OID for deleted/non-existent files.
    pub const NULL: &'static str = "0000000000000000";

    /// Compute OID from content (using fast hash, not SHA-1).
    pub fn from_content(content: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        Self(format!("{:016x}", hasher.finish()))
    }

    /// Compute OID from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self(format!("{:016x}", hasher.finish()))
    }

    /// Get null OID.
    pub fn null() -> Self {
        Self(Self::NULL.to_string())
    }

    /// Check if this is a null OID.
    pub fn is_null(&self) -> bool {
        self.0 == Self::NULL
    }

    /// Get short form (first 7 characters).
    pub fn short(&self) -> &str {
        &self.0[..7.min(self.0.len())]
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Baseline Snapshot
// ============================================================================

/// Snapshot of a file's state at the start of a turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// Path to the file.
    pub path: PathBuf,

    /// File mode at snapshot time.
    pub mode: FileMode,

    /// Object ID of content.
    pub oid: ObjectId,

    /// Full content (for undo).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Whether the file existed.
    pub exists: bool,

    /// Timestamp when snapshot was taken.
    pub timestamp: u64,
}

impl FileSnapshot {
    /// Take a snapshot of a file.
    pub fn capture(path: impl Into<PathBuf>, workspace_root: &Path) -> Self {
        let path = path.into();
        let full_path = if path.is_absolute() {
            path.clone()
        } else {
            workspace_root.join(&path)
        };

        if full_path.exists() && !full_path.is_dir() {
            let content = std::fs::read_to_string(&full_path).ok();
            let mode = FileMode::from_path(&full_path).unwrap_or_default();
            let oid = content
                .as_ref()
                .map(|c| ObjectId::from_content(c))
                .unwrap_or_else(ObjectId::null);

            Self {
                path,
                mode,
                oid,
                content,
                exists: true,
                timestamp: current_timestamp(),
            }
        } else {
            Self {
                path,
                mode: FileMode::default(),
                oid: ObjectId::null(),
                content: None,
                exists: false,
                timestamp: current_timestamp(),
            }
        }
    }

    /// Create a snapshot for a non-existent file.
    pub fn non_existent(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            mode: FileMode::default(),
            oid: ObjectId::null(),
            content: None,
            exists: false,
            timestamp: current_timestamp(),
        }
    }
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// File was created.
    Created,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
    /// File was renamed.
    Renamed,
}

impl ChangeType {
    /// Check if this change type creates a new file.
    pub fn creates_file(&self) -> bool {
        matches!(self, Self::Created)
    }

    /// Check if this change type removes a file.
    pub fn removes_file(&self) -> bool {
        matches!(self, Self::Deleted)
    }

    /// Check if this change type modifies content.
    pub fn modifies_content(&self) -> bool {
        matches!(self, Self::Modified | Self::Created)
    }
}

/// A single file change record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Path to the file (relative to workspace root).
    pub path: PathBuf,

    /// Type of change.
    pub change_type: ChangeType,

    /// Original content before the change (for undo).
    /// Only populated for modifications and deletions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,

    /// New content after the change.
    /// Only populated for creations and modifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_content: Option<String>,

    /// Original path for renames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<PathBuf>,

    /// Original file mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_mode: Option<FileMode>,

    /// New file mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_mode: Option<FileMode>,

    /// Original object ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_oid: Option<ObjectId>,

    /// New object ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_oid: Option<ObjectId>,

    /// Unix timestamp when the change was recorded.
    pub timestamp: u64,

    /// Tool call ID that made this change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl FileChange {
    /// Create a new file creation record.
    pub fn creation(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            change_type: ChangeType::Created,
            original_content: None,
            new_content: None,
            original_path: None,
            original_mode: None,
            new_mode: Some(FileMode::REGULAR),
            original_oid: None,
            new_oid: None,
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file creation record with content.
    pub fn creation_with_content(path: impl Into<PathBuf>, content: String) -> Self {
        let oid = ObjectId::from_content(&content);
        Self {
            path: path.into(),
            change_type: ChangeType::Created,
            original_content: None,
            new_content: Some(content),
            original_path: None,
            original_mode: None,
            new_mode: Some(FileMode::REGULAR),
            original_oid: None,
            new_oid: Some(oid),
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file modification record.
    pub fn modification(path: impl Into<PathBuf>, original: Option<String>) -> Self {
        let original_oid = original.as_ref().map(|c| ObjectId::from_content(c));
        Self {
            path: path.into(),
            change_type: ChangeType::Modified,
            original_content: original,
            new_content: None,
            original_path: None,
            original_mode: Some(FileMode::REGULAR),
            new_mode: Some(FileMode::REGULAR),
            original_oid,
            new_oid: None,
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file modification record with both contents.
    pub fn modification_with_content(
        path: impl Into<PathBuf>,
        original: Option<String>,
        new_content: String,
    ) -> Self {
        let original_oid = original.as_ref().map(|c| ObjectId::from_content(c));
        let new_oid = ObjectId::from_content(&new_content);
        Self {
            path: path.into(),
            change_type: ChangeType::Modified,
            original_content: original,
            new_content: Some(new_content),
            original_path: None,
            original_mode: Some(FileMode::REGULAR),
            new_mode: Some(FileMode::REGULAR),
            original_oid,
            new_oid: Some(new_oid),
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file deletion record.
    pub fn deletion(path: impl Into<PathBuf>, original: Option<String>) -> Self {
        let original_oid = original.as_ref().map(|c| ObjectId::from_content(c));
        Self {
            path: path.into(),
            change_type: ChangeType::Deleted,
            original_content: original,
            new_content: None,
            original_path: None,
            original_mode: Some(FileMode::REGULAR),
            new_mode: None,
            original_oid,
            new_oid: None,
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file rename record.
    pub fn rename(old_path: impl Into<PathBuf>, new_path: impl Into<PathBuf>) -> Self {
        Self {
            path: new_path.into(),
            change_type: ChangeType::Renamed,
            original_content: None,
            new_content: None,
            original_path: Some(old_path.into()),
            original_mode: Some(FileMode::REGULAR),
            new_mode: Some(FileMode::REGULAR),
            original_oid: None,
            new_oid: None,
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Set the tool call ID.
    pub fn with_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.tool_call_id = Some(id.into());
        self
    }

    /// Set the file modes.
    pub fn with_modes(mut self, original: Option<FileMode>, new: Option<FileMode>) -> Self {
        self.original_mode = original;
        self.new_mode = new;
        self
    }

    /// Set from a baseline snapshot.
    pub fn from_snapshot(mut self, snapshot: &FileSnapshot) -> Self {
        self.original_content = snapshot.content.clone();
        self.original_mode = Some(snapshot.mode);
        self.original_oid = Some(snapshot.oid.clone());
        self
    }

    /// Check if this change can be undone.
    pub fn can_undo(&self) -> bool {
        match self.change_type {
            ChangeType::Created => true, // Just delete
            ChangeType::Modified => self.original_content.is_some(),
            ChangeType::Deleted => self.original_content.is_some(),
            ChangeType::Renamed => self.original_path.is_some(),
        }
    }

    /// Check if file mode changed.
    pub fn mode_changed(&self) -> bool {
        match (&self.original_mode, &self.new_mode) {
            (Some(old), Some(new)) => old != new,
            _ => false,
        }
    }

    /// Count lines added and removed.
    pub fn line_counts(&self) -> (usize, usize) {
        let old_lines = self
            .original_content
            .as_ref()
            .map(|c| c.lines().count())
            .unwrap_or(0);
        let new_lines = self
            .new_content
            .as_ref()
            .map(|c| c.lines().count())
            .unwrap_or(0);

        match self.change_type {
            ChangeType::Created => (new_lines, 0),
            ChangeType::Deleted => (0, old_lines),
            ChangeType::Modified => {
                // Simple approximation: if we have both, show the difference
                if new_lines >= old_lines {
                    (new_lines - old_lines, 0)
                } else {
                    (0, old_lines - new_lines)
                }
            }
            ChangeType::Renamed => (0, 0),
        }
    }

    /// Generate unified diff for this change.
    pub fn unified_diff(&self) -> String {
        let path_str = self.path.display();
        let original_path_str = self
            .original_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| path_str.to_string());

        let old_oid = self
            .original_oid
            .as_ref()
            .map(|o| o.short())
            .unwrap_or(ObjectId::NULL);
        let new_oid = self
            .new_oid
            .as_ref()
            .map(|o| o.short())
            .unwrap_or(ObjectId::NULL);

        let old_mode = self
            .original_mode
            .as_ref()
            .map(|m| m.as_git_mode())
            .unwrap_or("000000");
        let new_mode = self
            .new_mode
            .as_ref()
            .map(|m| m.as_git_mode())
            .unwrap_or("000000");

        let mut diff = String::new();

        // Header
        match self.change_type {
            ChangeType::Created => {
                diff.push_str(&format!("diff --git a/{} b/{}\n", path_str, path_str));
                diff.push_str(&format!("new file mode {}\n", new_mode));
                diff.push_str(&format!(
                    "index {}..{}\n",
                    &ObjectId::NULL[..7],
                    new_oid
                ));
                diff.push_str(&format!("--- /dev/null\n"));
                diff.push_str(&format!("+++ b/{}\n", path_str));
            }
            ChangeType::Deleted => {
                diff.push_str(&format!("diff --git a/{} b/{}\n", path_str, path_str));
                diff.push_str(&format!("deleted file mode {}\n", old_mode));
                diff.push_str(&format!(
                    "index {}..{}\n",
                    old_oid,
                    &ObjectId::NULL[..7]
                ));
                diff.push_str(&format!("--- a/{}\n", path_str));
                diff.push_str("+++ /dev/null\n");
            }
            ChangeType::Modified => {
                diff.push_str(&format!("diff --git a/{} b/{}\n", path_str, path_str));
                if self.mode_changed() {
                    diff.push_str(&format!("old mode {}\n", old_mode));
                    diff.push_str(&format!("new mode {}\n", new_mode));
                }
                diff.push_str(&format!("index {}..{} {}\n", old_oid, new_oid, new_mode));
                diff.push_str(&format!("--- a/{}\n", path_str));
                diff.push_str(&format!("+++ b/{}\n", path_str));
            }
            ChangeType::Renamed => {
                diff.push_str(&format!(
                    "diff --git a/{} b/{}\n",
                    original_path_str, path_str
                ));
                diff.push_str(&format!("similarity index 100%\n"));
                diff.push_str(&format!("rename from {}\n", original_path_str));
                diff.push_str(&format!("rename to {}\n", path_str));
            }
        }

        // Content diff
        if self.change_type != ChangeType::Renamed {
            diff.push_str(&self.content_diff());
        }

        diff
    }

    /// Generate the content portion of the diff.
    fn content_diff(&self) -> String {
        let old_lines: Vec<&str> = self
            .original_content
            .as_ref()
            .map(|c| c.lines().collect())
            .unwrap_or_default();
        let new_lines: Vec<&str> = self
            .new_content
            .as_ref()
            .map(|c| c.lines().collect())
            .unwrap_or_default();

        if old_lines.is_empty() && new_lines.is_empty() {
            return String::new();
        }

        // Simple diff: show all old as removed, all new as added
        // For a proper diff algorithm, we'd use something like diff-match-patch
        let mut result = String::new();

        let old_len = old_lines.len();
        let new_len = new_lines.len();

        if old_len > 0 || new_len > 0 {
            result.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                if old_len > 0 { 1 } else { 0 },
                old_len,
                if new_len > 0 { 1 } else { 0 },
                new_len
            ));
        }

        for line in &old_lines {
            result.push_str(&format!("-{}\n", line));
        }

        for line in &new_lines {
            result.push_str(&format!("+{}\n", line));
        }

        result
    }
}

/// Summary of changes during a turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnDiff {
    /// Turn identifier.
    pub turn_id: String,

    /// All file changes in this turn.
    pub changes: Vec<FileChange>,

    /// Number of files created.
    pub files_created: usize,

    /// Number of files modified.
    pub files_modified: usize,

    /// Number of files deleted.
    pub files_deleted: usize,

    /// Number of files renamed.
    pub files_renamed: usize,

    /// Total lines added (approximate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_added: Option<usize>,

    /// Total lines removed (approximate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_removed: Option<usize>,

    /// Start timestamp of the turn.
    pub started_at: u64,

    /// End timestamp of the turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
}

impl TurnDiff {
    /// Create a new turn diff.
    pub fn new(turn_id: impl Into<String>) -> Self {
        Self {
            turn_id: turn_id.into(),
            started_at: current_timestamp(),
            ..Default::default()
        }
    }

    /// Check if any changes were made.
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Get total number of file changes.
    pub fn total_changes(&self) -> usize {
        self.changes.len()
    }

    /// Get paths of all changed files.
    pub fn changed_paths(&self) -> Vec<&Path> {
        self.changes.iter().map(|c| c.path.as_path()).collect()
    }

    /// Check if all changes can be undone.
    pub fn can_undo_all(&self) -> bool {
        self.changes.iter().all(|c| c.can_undo())
    }

    /// Get a human-readable summary.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if self.files_created > 0 {
            parts.push(format!(
                "{} file{} created",
                self.files_created,
                if self.files_created == 1 { "" } else { "s" }
            ));
        }

        if self.files_modified > 0 {
            parts.push(format!(
                "{} file{} modified",
                self.files_modified,
                if self.files_modified == 1 { "" } else { "s" }
            ));
        }

        if self.files_deleted > 0 {
            parts.push(format!(
                "{} file{} deleted",
                self.files_deleted,
                if self.files_deleted == 1 { "" } else { "s" }
            ));
        }

        if self.files_renamed > 0 {
            parts.push(format!(
                "{} file{} renamed",
                self.files_renamed,
                if self.files_renamed == 1 { "" } else { "s" }
            ));
        }

        if parts.is_empty() {
            "No file changes".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Generate unified diff for all changes.
    ///
    /// This produces a git-compatible unified diff format that can be
    /// used for visualization or applied with `git apply`.
    pub fn unified_diff(&self) -> String {
        let mut diff = String::new();

        for change in &self.changes {
            if !diff.is_empty() {
                diff.push('\n');
            }
            diff.push_str(&change.unified_diff());
        }

        diff
    }

    /// Generate a short diff stat (like `git diff --stat`).
    pub fn diff_stat(&self) -> String {
        let mut result = String::new();
        let mut max_path_len = 0;

        for change in &self.changes {
            let path_len = change.path.display().to_string().len();
            if path_len > max_path_len {
                max_path_len = path_len;
            }
        }

        for change in &self.changes {
            let path = change.path.display().to_string();
            let (added, removed) = change.line_counts();

            result.push_str(&format!(
                " {:<width$} | {:>4} {}{}\n",
                path,
                added + removed,
                "+".repeat(added.min(10)),
                "-".repeat(removed.min(10)),
                width = max_path_len
            ));

            // For renames, show the original path
            if change.change_type == ChangeType::Renamed {
                if let Some(ref orig) = change.original_path {
                    result.push_str(&format!(
                        " {:<width$} | (renamed from {})\n",
                        "",
                        orig.display(),
                        width = max_path_len
                    ));
                }
            }
        }

        let total_files = self.changes.len();
        let total_added: usize = self.changes.iter().map(|c| c.line_counts().0).sum();
        let total_removed: usize = self.changes.iter().map(|c| c.line_counts().1).sum();

        result.push_str(&format!(
            " {} file{} changed, {} insertion{}(+), {} deletion{}(-)\n",
            total_files,
            if total_files == 1 { "" } else { "s" },
            total_added,
            if total_added == 1 { "" } else { "s" },
            total_removed,
            if total_removed == 1 { "" } else { "s" }
        ));

        result
    }
}

/// Tracks file changes during a conversation turn.
///
/// This tracker records all file modifications made by tool calls
/// during a turn, enabling undo functionality.
///
/// ## Baseline Snapshots
///
/// The tracker captures file state before changes are made, enabling:
/// - Accurate undo with original content
/// - Unified diff generation with before/after comparison
/// - File mode tracking for permission changes
#[derive(Debug)]
pub struct TurnDiffTracker {
    /// Turn identifier.
    turn_id: String,

    /// Workspace root directory.
    workspace_root: PathBuf,

    /// Recorded changes indexed by path.
    changes: HashMap<PathBuf, FileChange>,

    /// Baseline snapshots of files before changes.
    baselines: HashMap<PathBuf, FileSnapshot>,

    /// Start time of the turn.
    started_at: Instant,

    /// Whether tracking is active.
    active: bool,

    /// Whether to capture file content in baselines.
    capture_content: bool,
}

impl TurnDiffTracker {
    /// Create a new turn diff tracker.
    pub fn new(turn_id: impl Into<String>, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            turn_id: turn_id.into(),
            workspace_root: workspace_root.into(),
            changes: HashMap::new(),
            baselines: HashMap::new(),
            started_at: Instant::now(),
            active: true,
            capture_content: true,
        }
    }

    /// Create a tracker without content capture (lighter weight).
    pub fn without_content(turn_id: impl Into<String>, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            turn_id: turn_id.into(),
            workspace_root: workspace_root.into(),
            changes: HashMap::new(),
            baselines: HashMap::new(),
            started_at: Instant::now(),
            active: true,
            capture_content: false,
        }
    }

    /// Get the turn ID.
    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }

    /// Get the workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Check if tracking is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Capture a baseline snapshot of a file before modification.
    ///
    /// This should be called before any modification to capture the original state.
    pub fn capture_baseline(&mut self, path: impl Into<PathBuf>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Only capture once per file per turn
        if self.baselines.contains_key(&path) {
            return;
        }

        let snapshot = if self.capture_content {
            FileSnapshot::capture(&path, &self.workspace_root)
        } else {
            // Capture metadata only
            let full_path = self.workspace_root.join(&path);
            if full_path.exists() {
                FileSnapshot {
                    path: path.clone(),
                    mode: FileMode::from_path(&full_path).unwrap_or_default(),
                    oid: ObjectId::null(),
                    content: None,
                    exists: true,
                    timestamp: current_timestamp(),
                }
            } else {
                FileSnapshot::non_existent(&path)
            }
        };

        self.baselines.insert(path, snapshot);
    }

    /// Get the baseline snapshot for a path.
    pub fn get_baseline(&self, path: &Path) -> Option<&FileSnapshot> {
        self.baselines.get(path)
    }

    /// Record a file creation.
    pub fn record_creation(&mut self, path: impl Into<PathBuf>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline (should be non-existent for creations)
        self.capture_baseline(&path);

        let change = FileChange::creation(&path);
        self.changes.insert(path, change);
    }

    /// Record a file creation with content.
    pub fn record_creation_with_content(&mut self, path: impl Into<PathBuf>, content: String) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline (should be non-existent for creations)
        self.capture_baseline(&path);

        let mode = FileMode::from_path(&self.workspace_root.join(&path));
        let mut change = FileChange::creation_with_content(&path, content);
        change.new_mode = mode.or(Some(FileMode::REGULAR));
        self.changes.insert(path, change);
    }

    /// Record a file creation with tool call ID.
    pub fn record_creation_with_tool(&mut self, path: impl Into<PathBuf>, tool_call_id: &str) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline
        self.capture_baseline(&path);

        let change = FileChange::creation(&path).with_tool_call_id(tool_call_id);
        self.changes.insert(path, change);
    }

    /// Record a file modification.
    ///
    /// If `original_content` is provided, the change can be undone.
    /// Prefer using `record_modification_with_content` for full diff support.
    pub fn record_modification(&mut self, path: impl Into<PathBuf>, original_content: Option<String>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline first (before any content changes)
        if !self.baselines.contains_key(&path) {
            self.capture_baseline(&path);
        }

        // Don't overwrite creation with modification
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                return;
            }
        }

        // Use baseline content if available and original_content not provided
        let original = original_content.or_else(|| {
            self.baselines.get(&path).and_then(|b| b.content.clone())
        });

        let baseline = self.baselines.get(&path);
        let mut change = FileChange::modification(&path, original);

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.original_oid = Some(baseline.oid.clone());
        }

        self.changes.insert(path, change);
    }

    /// Record a file modification with both original and new content.
    pub fn record_modification_with_content(
        &mut self,
        path: impl Into<PathBuf>,
        original_content: Option<String>,
        new_content: String,
    ) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline first
        if !self.baselines.contains_key(&path) {
            self.capture_baseline(&path);
        }

        // Don't overwrite creation with modification
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                // Update the new content on the creation record
                let mut updated = existing.clone();
                updated.new_content = Some(new_content.clone());
                updated.new_oid = Some(ObjectId::from_content(&new_content));
                self.changes.insert(path, updated);
                return;
            }
        }

        // Use baseline content if available and original_content not provided
        let original = original_content.or_else(|| {
            self.baselines.get(&path).and_then(|b| b.content.clone())
        });

        let baseline = self.baselines.get(&path);
        let mut change = FileChange::modification_with_content(&path, original, new_content);

        // Get current mode from filesystem
        let new_mode = FileMode::from_path(&self.workspace_root.join(&path));

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.original_oid = Some(baseline.oid.clone());
        }
        change.new_mode = new_mode.or(Some(FileMode::REGULAR));

        self.changes.insert(path, change);
    }

    /// Record a file modification with tool call ID.
    pub fn record_modification_with_tool(
        &mut self,
        path: impl Into<PathBuf>,
        original_content: Option<String>,
        tool_call_id: &str,
    ) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline first
        if !self.baselines.contains_key(&path) {
            self.capture_baseline(&path);
        }

        // Don't overwrite creation with modification
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                return;
            }
        }

        // Use baseline content if available
        let original = original_content.or_else(|| {
            self.baselines.get(&path).and_then(|b| b.content.clone())
        });

        let baseline = self.baselines.get(&path);
        let mut change = FileChange::modification(&path, original)
            .with_tool_call_id(tool_call_id);

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.original_oid = Some(baseline.oid.clone());
        }

        self.changes.insert(path, change);
    }

    /// Record a file deletion.
    ///
    /// If `original_content` is provided, the change can be undone.
    /// Otherwise, will use baseline content if available.
    pub fn record_deletion(&mut self, path: impl Into<PathBuf>, original_content: Option<String>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline first (before deletion)
        if !self.baselines.contains_key(&path) {
            self.capture_baseline(&path);
        }

        // If file was created in this turn, just remove the creation record
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                self.changes.remove(&path);
                return;
            }
        }

        // Use baseline content if available
        let original = original_content.or_else(|| {
            self.baselines.get(&path).and_then(|b| b.content.clone())
        });

        let baseline = self.baselines.get(&path);
        let mut change = FileChange::deletion(&path, original);

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.original_oid = Some(baseline.oid.clone());
        }

        self.changes.insert(path, change);
    }

    /// Record a file deletion with tool call ID.
    pub fn record_deletion_with_tool(
        &mut self,
        path: impl Into<PathBuf>,
        original_content: Option<String>,
        tool_call_id: &str,
    ) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Capture baseline first
        if !self.baselines.contains_key(&path) {
            self.capture_baseline(&path);
        }

        // If file was created in this turn, just remove the creation record
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                self.changes.remove(&path);
                return;
            }
        }

        // Use baseline content if available
        let original = original_content.or_else(|| {
            self.baselines.get(&path).and_then(|b| b.content.clone())
        });

        let baseline = self.baselines.get(&path);
        let mut change = FileChange::deletion(&path, original)
            .with_tool_call_id(tool_call_id);

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.original_oid = Some(baseline.oid.clone());
        }

        self.changes.insert(path, change);
    }

    /// Record a file rename.
    pub fn record_rename(&mut self, old_path: impl Into<PathBuf>, new_path: impl Into<PathBuf>) {
        if !self.active {
            return;
        }

        let old_path = self.normalize_path(old_path.into());
        let new_path = self.normalize_path(new_path.into());

        // Capture baseline of old path
        if !self.baselines.contains_key(&old_path) {
            self.capture_baseline(&old_path);
        }

        // Remove old path record if exists
        self.changes.remove(&old_path);

        let baseline = self.baselines.get(&old_path);
        let mut change = FileChange::rename(&old_path, &new_path);

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.new_mode = Some(baseline.mode); // Same mode after rename
            change.original_oid = Some(baseline.oid.clone());
            change.new_oid = Some(baseline.oid.clone()); // Same content after rename
        }

        self.changes.insert(new_path, change);
    }

    /// Record a file rename with tool call ID.
    pub fn record_rename_with_tool(
        &mut self,
        old_path: impl Into<PathBuf>,
        new_path: impl Into<PathBuf>,
        tool_call_id: &str,
    ) {
        if !self.active {
            return;
        }

        let old_path = self.normalize_path(old_path.into());
        let new_path = self.normalize_path(new_path.into());

        // Capture baseline of old path
        if !self.baselines.contains_key(&old_path) {
            self.capture_baseline(&old_path);
        }

        // Remove old path record if exists
        self.changes.remove(&old_path);

        let baseline = self.baselines.get(&old_path);
        let mut change = FileChange::rename(&old_path, &new_path)
            .with_tool_call_id(tool_call_id);

        if let Some(baseline) = baseline {
            change.original_mode = Some(baseline.mode);
            change.new_mode = Some(baseline.mode);
            change.original_oid = Some(baseline.oid.clone());
            change.new_oid = Some(baseline.oid.clone());
        }

        self.changes.insert(new_path, change);
    }

    /// Stop tracking and return the accumulated diff.
    pub fn finish(mut self) -> TurnDiff {
        self.active = false;
        self.build_diff()
    }

    /// Build the current diff without stopping tracking.
    pub fn current_diff(&self) -> TurnDiff {
        self.build_diff()
    }

    /// Check if any changes have been recorded.
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Get the number of recorded changes.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Revert all changes made during this turn.
    ///
    /// This attempts to undo all recorded changes in reverse order.
    /// Returns the number of successfully reverted changes.
    pub async fn revert(&self) -> GitResult<usize> {
        let mut reverted = 0;

        // Sort changes by timestamp (newest first for proper undo order)
        let mut sorted_changes: Vec<_> = self.changes.values().collect();
        sorted_changes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        for change in sorted_changes {
            if self.revert_change(change).await? {
                reverted += 1;
            }
        }

        Ok(reverted)
    }

    /// Revert a single change.
    async fn revert_change(&self, change: &FileChange) -> GitResult<bool> {
        let full_path = self.workspace_root.join(&change.path);

        match change.change_type {
            ChangeType::Created => {
                // Delete the created file
                if full_path.exists() {
                    tokio::fs::remove_file(&full_path).await
                        .map_err(|e| crate::GitError::Io(e))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ChangeType::Modified => {
                // Restore original content
                if let Some(ref original) = change.original_content {
                    tokio::fs::write(&full_path, original).await
                        .map_err(|e| crate::GitError::Io(e))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ChangeType::Deleted => {
                // Recreate the file with original content
                if let Some(ref original) = change.original_content {
                    // Ensure parent directory exists
                    if let Some(parent) = full_path.parent() {
                        tokio::fs::create_dir_all(parent).await
                            .map_err(|e| crate::GitError::Io(e))?;
                    }
                    tokio::fs::write(&full_path, original).await
                        .map_err(|e| crate::GitError::Io(e))?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ChangeType::Renamed => {
                // Rename back to original path
                if let Some(ref original_path) = change.original_path {
                    let old_full_path = self.workspace_root.join(original_path);
                    if full_path.exists() {
                        tokio::fs::rename(&full_path, &old_full_path).await
                            .map_err(|e| crate::GitError::Io(e))?;
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Normalize a path relative to workspace root.
    fn normalize_path(&self, path: PathBuf) -> PathBuf {
        if path.is_absolute() {
            path.strip_prefix(&self.workspace_root)
                .map(|p| p.to_path_buf())
                .unwrap_or(path)
        } else {
            path
        }
    }

    /// Build a TurnDiff from current state.
    fn build_diff(&self) -> TurnDiff {
        let mut diff = TurnDiff::new(&self.turn_id);
        diff.started_at = current_timestamp() - self.started_at.elapsed().as_secs();

        let lines_added = 0usize;
        let mut lines_removed = 0usize;

        for change in self.changes.values() {
            match change.change_type {
                ChangeType::Created => diff.files_created += 1,
                ChangeType::Modified => diff.files_modified += 1,
                ChangeType::Deleted => diff.files_deleted += 1,
                ChangeType::Renamed => diff.files_renamed += 1,
            }

            // Estimate line changes
            if let Some(ref original) = change.original_content {
                lines_removed += original.lines().count();
            }
            // We don't have the new content here, so we can't calculate lines_added accurately

            diff.changes.push(change.clone());
        }

        if lines_removed > 0 {
            diff.lines_removed = Some(lines_removed);
        }
        if lines_added > 0 {
            diff.lines_added = Some(lines_added);
        }

        diff.ended_at = Some(current_timestamp());
        diff
    }
}

/// Shared turn diff tracker for concurrent access.
pub type SharedTurnDiffTracker = Arc<RwLock<Option<TurnDiffTracker>>>;

/// Create a new shared tracker.
pub fn new_shared_tracker(turn_id: impl Into<String>, workspace_root: impl Into<PathBuf>) -> SharedTurnDiffTracker {
    Arc::new(RwLock::new(Some(TurnDiffTracker::new(turn_id, workspace_root))))
}

/// Get current Unix timestamp.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_change_types() {
        assert!(ChangeType::Created.creates_file());
        assert!(!ChangeType::Modified.creates_file());
        assert!(ChangeType::Deleted.removes_file());
        assert!(!ChangeType::Created.removes_file());
    }

    #[test]
    fn test_file_change_creation() {
        let change = FileChange::creation("test.rs");
        assert_eq!(change.change_type, ChangeType::Created);
        assert!(change.can_undo());
    }

    #[test]
    fn test_file_change_modification() {
        let change = FileChange::modification("test.rs", Some("original".to_string()));
        assert_eq!(change.change_type, ChangeType::Modified);
        assert!(change.can_undo());

        let change_no_original = FileChange::modification("test.rs", None);
        assert!(!change_no_original.can_undo());
    }

    #[test]
    fn test_tracker_record_changes() {
        let tracker = TurnDiffTracker::new("test_turn", "/workspace");

        let mut tracker = tracker;
        tracker.record_creation("new_file.rs");
        tracker.record_modification("existing.rs", Some("old content".to_string()));
        tracker.record_deletion("deleted.rs", Some("deleted content".to_string()));

        assert_eq!(tracker.change_count(), 3);
        assert!(tracker.has_changes());
    }

    #[test]
    fn test_tracker_creation_then_deletion() {
        let mut tracker = TurnDiffTracker::new("test_turn", "/workspace");

        // Create a file
        tracker.record_creation("temp.rs");
        assert_eq!(tracker.change_count(), 1);

        // Delete the same file - should cancel out
        tracker.record_deletion("temp.rs", None);
        assert_eq!(tracker.change_count(), 0);
    }

    #[test]
    fn test_turn_diff_summary() {
        let mut diff = TurnDiff::new("test");
        diff.files_created = 2;
        diff.files_modified = 1;

        let summary = diff.summary();
        assert!(summary.contains("2 files created"));
        assert!(summary.contains("1 file modified"));
    }

    #[tokio::test]
    async fn test_revert_creation() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();

        // Create a file
        let file_path = workspace.join("test.txt");
        tokio::fs::write(&file_path, "content").await.unwrap();
        assert!(file_path.exists());

        // Track the creation
        let mut tracker = TurnDiffTracker::new("test", workspace);
        tracker.record_creation("test.txt");

        // Revert
        let reverted = tracker.revert().await.unwrap();
        assert_eq!(reverted, 1);
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_revert_modification() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();

        // Create a file with original content
        let file_path = workspace.join("test.txt");
        tokio::fs::write(&file_path, "original").await.unwrap();

        // Track modification
        let mut tracker = TurnDiffTracker::new("test", workspace);
        tracker.record_modification("test.txt", Some("original".to_string()));

        // Modify the file
        tokio::fs::write(&file_path, "modified").await.unwrap();

        // Revert
        let reverted = tracker.revert().await.unwrap();
        assert_eq!(reverted, 1);

        // Check content restored
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "original");
    }
}
