//! Integration tests for turn diff tracker with file handlers.

use std::sync::Arc;
use tempfile::TempDir;
use serde_json::json;

use mms_config::Config;
use mms_git::turn_diff::new_shared_tracker;
use mms_tools::{ToolContext, ToolHandler, ToolCall};
use mms_tools::handlers::{WriteFileHandler, EditFileHandler};

fn create_test_context_with_tracker(temp_dir: &TempDir) -> ToolContext {
    let config = Config::default();
    let mut ctx = ToolContext::new(Arc::new(config));
    ctx.set_cwd(temp_dir.path().to_path_buf());

    // Add diff tracker
    let tracker = new_shared_tracker("test_turn", temp_dir.path());
    ctx.with_diff_tracker(tracker)
}

#[tokio::test]
async fn test_write_file_records_creation() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context_with_tracker(&temp_dir);
    let handler = WriteFileHandler;

    let call = ToolCall::new(
        "test-1",
        "write_file",
        json!({
            "path": "new_file.txt",
            "content": "Hello, World!"
        }),
    );

    let result = handler.execute(&ctx, call).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Created"));

    // Verify the tracker recorded the creation
    let tracker = ctx.diff_tracker().unwrap();
    let guard = tracker.read().await;
    let tracker_inner = guard.as_ref().unwrap();

    assert_eq!(tracker_inner.change_count(), 1);

    // Get current diff snapshot
    let diff = tracker_inner.current_diff();
    assert_eq!(diff.files_created, 1);
    assert_eq!(diff.changes.len(), 1);

    let change = &diff.changes[0];
    assert!(change.path.ends_with("new_file.txt"));
    assert_eq!(change.change_type, mms_git::turn_diff::ChangeType::Created);
    assert_eq!(change.tool_call_id, Some("test-1".to_string()));
}

#[tokio::test]
async fn test_write_file_records_modification() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("existing.txt");
    std::fs::write(&file_path, "Original content").unwrap();

    let ctx = create_test_context_with_tracker(&temp_dir);
    let handler = WriteFileHandler;

    let call = ToolCall::new(
        "test-2",
        "write_file",
        json!({
            "path": "existing.txt",
            "content": "New content"
        }),
    );

    let result = handler.execute(&ctx, call).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Updated"));

    // Verify the tracker recorded the modification
    let tracker = ctx.diff_tracker().unwrap();
    let guard = tracker.read().await;
    let tracker_inner = guard.as_ref().unwrap();

    assert_eq!(tracker_inner.change_count(), 1);

    let diff = tracker_inner.current_diff();
    assert_eq!(diff.files_modified, 1);
    let change = &diff.changes[0];
    assert!(change.path.ends_with("existing.txt"));
    assert_eq!(change.change_type, mms_git::turn_diff::ChangeType::Modified);
    assert_eq!(change.original_content, Some("Original content".to_string()));
}

#[tokio::test]
async fn test_edit_file_records_modification() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("code.rs");
    std::fs::write(&file_path, "fn hello() {\n    println!(\"Hello\");\n}").unwrap();

    let ctx = create_test_context_with_tracker(&temp_dir);
    let handler = EditFileHandler;

    let call = ToolCall::new(
        "test-3",
        "edit_file",
        json!({
            "path": "code.rs",
            "old_string": "Hello",
            "new_string": "World"
        }),
    );

    let result = handler.execute(&ctx, call).await.unwrap();
    assert!(result.success);

    // Verify the tracker recorded the modification
    let tracker = ctx.diff_tracker().unwrap();
    let guard = tracker.read().await;
    let tracker_inner = guard.as_ref().unwrap();

    assert_eq!(tracker_inner.change_count(), 1);

    let diff = tracker_inner.current_diff();
    assert_eq!(diff.files_modified, 1);
    let change = &diff.changes[0];
    assert!(change.path.ends_with("code.rs"));
    assert_eq!(change.change_type, mms_git::turn_diff::ChangeType::Modified);
    assert_eq!(change.original_content, Some("fn hello() {\n    println!(\"Hello\");\n}".to_string()));
}

#[tokio::test]
async fn test_multiple_operations_tracked() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("file1.txt");
    std::fs::write(&file_path, "Content 1").unwrap();

    let ctx = create_test_context_with_tracker(&temp_dir);
    let write_handler = WriteFileHandler;
    let edit_handler = EditFileHandler;

    // Create a new file
    let call1 = ToolCall::new(
        "call-1",
        "write_file",
        json!({
            "path": "new_file.txt",
            "content": "New content"
        }),
    );
    write_handler.execute(&ctx, call1).await.unwrap();

    // Edit existing file
    let call2 = ToolCall::new(
        "call-2",
        "edit_file",
        json!({
            "path": "file1.txt",
            "old_string": "Content 1",
            "new_string": "Modified content"
        }),
    );
    edit_handler.execute(&ctx, call2).await.unwrap();

    // Create another file
    let call3 = ToolCall::new(
        "call-3",
        "write_file",
        json!({
            "path": "file2.txt",
            "content": "File 2 content"
        }),
    );
    write_handler.execute(&ctx, call3).await.unwrap();

    // Verify tracker recorded all operations
    let tracker = ctx.diff_tracker().unwrap();
    let guard = tracker.read().await;
    let tracker_inner = guard.as_ref().unwrap();

    assert_eq!(tracker_inner.change_count(), 3);

    // Verify we can get summary using current_diff
    let diff = tracker_inner.current_diff();
    assert_eq!(diff.files_created, 2);
    assert_eq!(diff.files_modified, 1);
    assert_eq!(diff.total_changes(), 3);
}

#[tokio::test]
async fn test_no_tracker_still_works() {
    // Test that handlers work when no tracker is configured
    let temp_dir = TempDir::new().unwrap();
    let config = Config::default();
    let mut ctx = ToolContext::new(Arc::new(config));
    ctx.set_cwd(temp_dir.path().to_path_buf());
    // No tracker configured

    let handler = WriteFileHandler;

    let call = ToolCall::new(
        "test-1",
        "write_file",
        json!({
            "path": "file.txt",
            "content": "Content"
        }),
    );

    let result = handler.execute(&ctx, call).await.unwrap();
    assert!(result.success);

    // File should still be written
    let content = std::fs::read_to_string(temp_dir.path().join("file.txt")).unwrap();
    assert_eq!(content, "Content");
}
