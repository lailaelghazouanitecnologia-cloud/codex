//! Undo task for reverting file changes from previous turns.
//!
//! This module provides functionality to revert file changes tracked
//! by the TurnDiffTracker, enabling users to undo modifications made
//! during conversation turns.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use mms_git::turn_diff::{ChangeType, FileChange, SharedTurnDiffTracker, TurnDiff};

use crate::{TaskError, TaskResult};

/// Result of an undo operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    /// Files that were successfully reverted.
    pub reverted: Vec<RevertedFile>,

    /// Files that failed to revert.
    pub failed: Vec<FailedRevert>,

    /// Total number of changes that were undone.
    pub changes_undone: usize,

    /// Summary of the undo operation.
    pub summary: String,
}

/// A file that was successfully reverted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertedFile {
    /// Path to the file.
    pub path: PathBuf,

    /// Type of revert performed.
    pub revert_type: RevertType,
}

/// Type of revert operation performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevertType {
    /// Deleted a created file.
    Deleted,
    /// Restored original content.
    Restored,
    /// Recreated a deleted file.
    Recreated,
    /// Renamed back to original path.
    RenamedBack,
}

impl RevertType {
    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Deleted => "Deleted",
            Self::Restored => "Restored original content",
            Self::Recreated => "Recreated",
            Self::RenamedBack => "Renamed back",
        }
    }
}

/// A file that failed to revert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedRevert {
    /// Path to the file.
    pub path: PathBuf,

    /// Error message.
    pub error: String,
}

/// Configuration for undo operations.
#[derive(Debug, Clone, Default)]
pub struct UndoConfig {
    /// Only revert specific files (empty means all).
    pub file_filter: Vec<PathBuf>,

    /// Create backups before reverting.
    pub create_backups: bool,

    /// Dry run - don't actually make changes.
    pub dry_run: bool,
}

impl UndoConfig {
    /// Create a new undo config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set file filter.
    pub fn with_files(mut self, files: Vec<PathBuf>) -> Self {
        self.file_filter = files;
        self
    }

    /// Enable backups.
    pub fn with_backups(mut self) -> Self {
        self.create_backups = true;
        self
    }

    /// Enable dry run mode.
    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }
}

/// Undo task that reverts changes from a turn.
pub struct UndoTask {
    /// The diff to undo.
    diff: TurnDiff,

    /// Configuration for the undo.
    config: UndoConfig,
}

impl UndoTask {
    /// Create a new undo task from a turn diff.
    pub fn new(diff: TurnDiff) -> Self {
        Self {
            diff,
            config: UndoConfig::default(),
        }
    }

    /// Create with configuration.
    pub fn with_config(diff: TurnDiff, config: UndoConfig) -> Self {
        Self { diff, config }
    }

    /// Execute the undo operation.
    pub async fn execute(&self) -> TaskResult<UndoResult> {
        let changes = self.filter_changes();

        if changes.is_empty() {
            return Ok(UndoResult {
                reverted: vec![],
                failed: vec![],
                changes_undone: 0,
                summary: "No changes to undo".to_string(),
            });
        }

        let mut reverted = Vec::new();
        let mut failed = Vec::new();

        for change in changes {
            if self.config.dry_run {
                debug!("Dry run: would revert {:?}", change.path);
                reverted.push(RevertedFile {
                    path: change.path.clone(),
                    revert_type: self.revert_type_for(&change),
                });
                continue;
            }

            match self.revert_change(&change).await {
                Ok(revert_type) => {
                    info!("Reverted {:?}: {}", change.path, revert_type.description());
                    reverted.push(RevertedFile {
                        path: change.path.clone(),
                        revert_type,
                    });
                }
                Err(e) => {
                    warn!("Failed to revert {:?}: {}", change.path, e);
                    failed.push(FailedRevert {
                        path: change.path.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        let changes_undone = reverted.len();
        let summary = self.build_summary(&reverted, &failed);

        Ok(UndoResult {
            reverted,
            failed,
            changes_undone,
            summary,
        })
    }

    /// Filter changes based on config.
    fn filter_changes(&self) -> Vec<&FileChange> {
        if self.config.file_filter.is_empty() {
            self.diff.changes.iter().collect()
        } else {
            self.diff
                .changes
                .iter()
                .filter(|c| self.config.file_filter.iter().any(|f| c.path.ends_with(f)))
                .collect()
        }
    }

    /// Get the revert type for a change.
    fn revert_type_for(&self, change: &FileChange) -> RevertType {
        match change.change_type {
            ChangeType::Created => RevertType::Deleted,
            ChangeType::Modified => RevertType::Restored,
            ChangeType::Deleted => RevertType::Recreated,
            ChangeType::Renamed => RevertType::RenamedBack,
        }
    }

    /// Revert a single change.
    async fn revert_change(&self, change: &FileChange) -> TaskResult<RevertType> {
        match change.change_type {
            ChangeType::Created => self.revert_creation(change).await,
            ChangeType::Modified => self.revert_modification(change).await,
            ChangeType::Deleted => self.revert_deletion(change).await,
            ChangeType::Renamed => self.revert_rename(change).await,
        }
    }

    /// Revert a file creation by deleting the file.
    async fn revert_creation(&self, change: &FileChange) -> TaskResult<RevertType> {
        let path = &change.path;

        if !path.exists() {
            debug!("File already deleted: {:?}", path);
            return Ok(RevertType::Deleted);
        }

        if self.config.create_backups {
            self.create_backup(path).await?;
        }

        tokio::fs::remove_file(path)
            .await
            .map_err(|e| TaskError::io(format!("Failed to delete file: {}", e)))?;

        Ok(RevertType::Deleted)
    }

    /// Revert a file modification by restoring original content.
    async fn revert_modification(&self, change: &FileChange) -> TaskResult<RevertType> {
        let path = &change.path;

        let original = change.original_content.as_ref().ok_or_else(|| {
            TaskError::invalid_input("Cannot revert: original content not available")
        })?;

        if self.config.create_backups {
            self.create_backup(path).await?;
        }

        tokio::fs::write(path, original)
            .await
            .map_err(|e| TaskError::io(format!("Failed to restore file: {}", e)))?;

        Ok(RevertType::Restored)
    }

    /// Revert a file deletion by recreating the file.
    async fn revert_deletion(&self, change: &FileChange) -> TaskResult<RevertType> {
        let path = &change.path;

        let original = change.original_content.as_ref().ok_or_else(|| {
            TaskError::invalid_input("Cannot revert: original content not available")
        })?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| TaskError::io(format!("Failed to create directory: {}", e)))?;
            }
        }

        tokio::fs::write(path, original)
            .await
            .map_err(|e| TaskError::io(format!("Failed to recreate file: {}", e)))?;

        Ok(RevertType::Recreated)
    }

    /// Revert a file rename by renaming back.
    async fn revert_rename(&self, change: &FileChange) -> TaskResult<RevertType> {
        let current_path = &change.path;
        let original_path = change.original_path.as_ref().ok_or_else(|| {
            TaskError::invalid_input("Cannot revert: original path not available")
        })?;

        if !current_path.exists() {
            return Err(TaskError::not_found(format!(
                "File not found: {:?}",
                current_path
            )));
        }

        // Ensure parent directory of original path exists
        if let Some(parent) = original_path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| TaskError::io(format!("Failed to create directory: {}", e)))?;
            }
        }

        tokio::fs::rename(current_path, original_path)
            .await
            .map_err(|e| TaskError::io(format!("Failed to rename file: {}", e)))?;

        Ok(RevertType::RenamedBack)
    }

    /// Create a backup of a file.
    async fn create_backup(&self, path: &Path) -> TaskResult<()> {
        if !path.exists() {
            return Ok(());
        }

        let backup_path = path.with_extension(format!(
            "{}.backup",
            path.extension().unwrap_or_default().to_string_lossy()
        ));

        tokio::fs::copy(path, &backup_path)
            .await
            .map_err(|e| TaskError::io(format!("Failed to create backup: {}", e)))?;

        debug!("Created backup: {:?}", backup_path);
        Ok(())
    }

    /// Build a summary of the undo operation.
    fn build_summary(&self, reverted: &[RevertedFile], failed: &[FailedRevert]) -> String {
        let mut parts = Vec::new();

        if !reverted.is_empty() {
            let deleted = reverted.iter().filter(|r| r.revert_type == RevertType::Deleted).count();
            let restored = reverted.iter().filter(|r| r.revert_type == RevertType::Restored).count();
            let recreated = reverted.iter().filter(|r| r.revert_type == RevertType::Recreated).count();
            let renamed = reverted.iter().filter(|r| r.revert_type == RevertType::RenamedBack).count();

            if deleted > 0 {
                parts.push(format!("{} file{} deleted", deleted, if deleted == 1 { "" } else { "s" }));
            }
            if restored > 0 {
                parts.push(format!("{} file{} restored", restored, if restored == 1 { "" } else { "s" }));
            }
            if recreated > 0 {
                parts.push(format!("{} file{} recreated", recreated, if recreated == 1 { "" } else { "s" }));
            }
            if renamed > 0 {
                parts.push(format!("{} file{} renamed back", renamed, if renamed == 1 { "" } else { "s" }));
            }
        }

        if !failed.is_empty() {
            parts.push(format!("{} failed", failed.len()));
        }

        if parts.is_empty() {
            "No changes made".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Undo changes from a shared tracker.
pub async fn undo_from_tracker(
    tracker: &SharedTurnDiffTracker,
    config: UndoConfig,
) -> TaskResult<UndoResult> {
    let diff = {
        let guard = tracker.read().await;
        match guard.as_ref() {
            Some(t) => t.current_diff(),
            None => {
                return Ok(UndoResult {
                    reverted: vec![],
                    failed: vec![],
                    changes_undone: 0,
                    summary: "No active turn to undo".to_string(),
                });
            }
        }
    };

    let task = UndoTask::with_config(diff, config);
    task.execute().await
}

/// Preview what would be undone without making changes.
pub async fn preview_undo(tracker: &SharedTurnDiffTracker) -> TaskResult<UndoResult> {
    let config = UndoConfig::new().dry_run();
    undo_from_tracker(tracker, config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use mms_git::turn_diff::{FileChange, TurnDiff};
    use tempfile::TempDir;

    fn create_test_diff() -> TurnDiff {
        TurnDiff {
            turn_id: "test_turn".to_string(),
            changes: vec![],
            files_created: 0,
            files_modified: 0,
            files_deleted: 0,
            files_renamed: 0,
            lines_added: None,
            lines_removed: None,
            started_at: 0,
            ended_at: None,
        }
    }

    #[tokio::test]
    async fn test_undo_empty_diff() {
        let diff = create_test_diff();
        let task = UndoTask::new(diff);
        let result = task.execute().await.unwrap();

        assert_eq!(result.changes_undone, 0);
        assert!(result.reverted.is_empty());
        assert!(result.failed.is_empty());
    }

    #[tokio::test]
    async fn test_undo_created_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");
        std::fs::write(&file_path, "new content").unwrap();

        let mut diff = create_test_diff();
        diff.changes.push(FileChange::creation(&file_path));
        diff.files_created = 1;

        let task = UndoTask::new(diff);
        let result = task.execute().await.unwrap();

        assert_eq!(result.changes_undone, 1);
        assert!(!file_path.exists());
        assert_eq!(result.reverted[0].revert_type, RevertType::Deleted);
    }

    #[tokio::test]
    async fn test_undo_modified_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("existing.txt");
        std::fs::write(&file_path, "modified content").unwrap();

        let mut diff = create_test_diff();
        diff.changes.push(FileChange::modification(
            &file_path,
            Some("original content".to_string()),
        ));
        diff.files_modified = 1;

        let task = UndoTask::new(diff);
        let result = task.execute().await.unwrap();

        assert_eq!(result.changes_undone, 1);
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "original content");
        assert_eq!(result.reverted[0].revert_type, RevertType::Restored);
    }

    #[tokio::test]
    async fn test_undo_deleted_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("deleted.txt");

        let mut diff = create_test_diff();
        diff.changes.push(FileChange::deletion(
            &file_path,
            Some("original content".to_string()),
        ));
        diff.files_deleted = 1;

        let task = UndoTask::new(diff);
        let result = task.execute().await.unwrap();

        assert_eq!(result.changes_undone, 1);
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "original content");
        assert_eq!(result.reverted[0].revert_type, RevertType::Recreated);
    }

    #[tokio::test]
    async fn test_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let mut diff = create_test_diff();
        diff.changes.push(FileChange::creation(&file_path));
        diff.files_created = 1;

        let config = UndoConfig::new().dry_run();
        let task = UndoTask::with_config(diff, config);
        let result = task.execute().await.unwrap();

        // File should still exist in dry run mode
        assert!(file_path.exists());
        assert_eq!(result.changes_undone, 1);
    }

    #[tokio::test]
    async fn test_file_filter() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("keep.txt");
        let file2 = temp_dir.path().join("delete.txt");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();

        let mut diff = create_test_diff();
        diff.changes.push(FileChange::creation(&file1));
        diff.changes.push(FileChange::creation(&file2));
        diff.files_created = 2;

        let config = UndoConfig::new().with_files(vec![PathBuf::from("delete.txt")]);
        let task = UndoTask::with_config(diff, config);
        let result = task.execute().await.unwrap();

        // Only delete.txt should be reverted
        assert!(file1.exists());
        assert!(!file2.exists());
        assert_eq!(result.changes_undone, 1);
    }
}
