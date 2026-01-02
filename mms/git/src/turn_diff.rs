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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::GitResult;

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

    /// Original path for renames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<PathBuf>,

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
            original_path: None,
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file modification record.
    pub fn modification(path: impl Into<PathBuf>, original: Option<String>) -> Self {
        Self {
            path: path.into(),
            change_type: ChangeType::Modified,
            original_content: original,
            original_path: None,
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Create a new file deletion record.
    pub fn deletion(path: impl Into<PathBuf>, original: Option<String>) -> Self {
        Self {
            path: path.into(),
            change_type: ChangeType::Deleted,
            original_content: original,
            original_path: None,
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
            original_path: Some(old_path.into()),
            timestamp: current_timestamp(),
            tool_call_id: None,
        }
    }

    /// Set the tool call ID.
    pub fn with_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.tool_call_id = Some(id.into());
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
}

/// Tracks file changes during a conversation turn.
///
/// This tracker records all file modifications made by tool calls
/// during a turn, enabling undo functionality.
#[derive(Debug)]
pub struct TurnDiffTracker {
    /// Turn identifier.
    turn_id: String,

    /// Workspace root directory.
    workspace_root: PathBuf,

    /// Recorded changes indexed by path.
    changes: HashMap<PathBuf, FileChange>,

    /// Start time of the turn.
    started_at: Instant,

    /// Whether tracking is active.
    active: bool,
}

impl TurnDiffTracker {
    /// Create a new turn diff tracker.
    pub fn new(turn_id: impl Into<String>, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            turn_id: turn_id.into(),
            workspace_root: workspace_root.into(),
            changes: HashMap::new(),
            started_at: Instant::now(),
            active: true,
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

    /// Record a file creation.
    pub fn record_creation(&mut self, path: impl Into<PathBuf>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());
        let change = FileChange::creation(&path);
        self.changes.insert(path, change);
    }

    /// Record a file creation with tool call ID.
    pub fn record_creation_with_tool(&mut self, path: impl Into<PathBuf>, tool_call_id: &str) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());
        let change = FileChange::creation(&path).with_tool_call_id(tool_call_id);
        self.changes.insert(path, change);
    }

    /// Record a file modification.
    ///
    /// If `original_content` is provided, the change can be undone.
    pub fn record_modification(&mut self, path: impl Into<PathBuf>, original_content: Option<String>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // Don't overwrite creation with modification
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                return;
            }
        }

        let change = FileChange::modification(&path, original_content);
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

        // Don't overwrite creation with modification
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                return;
            }
        }

        let change = FileChange::modification(&path, original_content)
            .with_tool_call_id(tool_call_id);
        self.changes.insert(path, change);
    }

    /// Record a file deletion.
    ///
    /// If `original_content` is provided, the change can be undone.
    pub fn record_deletion(&mut self, path: impl Into<PathBuf>, original_content: Option<String>) {
        if !self.active {
            return;
        }

        let path = self.normalize_path(path.into());

        // If file was created in this turn, just remove the creation record
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                self.changes.remove(&path);
                return;
            }
        }

        let change = FileChange::deletion(&path, original_content);
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

        // If file was created in this turn, just remove the creation record
        if let Some(existing) = self.changes.get(&path) {
            if existing.change_type == ChangeType::Created {
                self.changes.remove(&path);
                return;
            }
        }

        let change = FileChange::deletion(&path, original_content)
            .with_tool_call_id(tool_call_id);
        self.changes.insert(path, change);
    }

    /// Record a file rename.
    pub fn record_rename(&mut self, old_path: impl Into<PathBuf>, new_path: impl Into<PathBuf>) {
        if !self.active {
            return;
        }

        let old_path = self.normalize_path(old_path.into());
        let new_path = self.normalize_path(new_path.into());

        // Remove old path record if exists
        self.changes.remove(&old_path);

        let change = FileChange::rename(&old_path, &new_path);
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

        // Remove old path record if exists
        self.changes.remove(&old_path);

        let change = FileChange::rename(&old_path, &new_path)
            .with_tool_call_id(tool_call_id);
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
