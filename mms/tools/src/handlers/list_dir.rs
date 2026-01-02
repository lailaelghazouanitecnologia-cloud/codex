//! List directory handler for viewing directory contents.

use serde_json::json;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Handler for listing directory contents.
pub struct ListDirectoryHandler;

impl ToolHandler for ListDirectoryHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "list_directory",
            "List the contents of a directory, showing files and subdirectories.",
        )
        .with_parameters(
            json!({
                "path": {
                    "type": "string",
                    "description": "The path to the directory to list. Defaults to current directory if not specified.",
                    "default": "."
                },
                "show_hidden": {
                    "type": "boolean",
                    "description": "Include hidden files (starting with .) in the output. Default is false.",
                    "default": false
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Recursively list subdirectories. Default is false.",
                    "default": false
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth for recursive listing. Only used if recursive is true. Default is 3.",
                    "default": 3
                }
            }),
            vec![], // No required parameters, path defaults to "."
        )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let path = call.get_string("path").unwrap_or_else(|| ".".to_string());
        let show_hidden = call.get_bool("show_hidden").unwrap_or(false);
        let recursive = call.get_bool("recursive").unwrap_or(false);
        let max_depth = call.get_i64("max_depth").unwrap_or(3) as usize;
        let resolved_path = ctx.resolve_path(&path);
        let call_id = call.id.clone();

        Box::pin(async move {
            let start = Instant::now();

            // Check if path exists and is a directory
            let metadata = tokio::fs::metadata(&resolved_path).await.map_err(|e| {
                mms_common::AgentError::Io { source: e }
            })?;

            if !metadata.is_dir() {
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ToolOutput::error(
                    call_id,
                    format!("{} is not a directory", resolved_path.display()),
                    duration_ms,
                ));
            }

            // List the directory
            let mut entries = Vec::new();
            list_directory_recursive(
                &resolved_path,
                &resolved_path,
                show_hidden,
                recursive,
                max_depth,
                0,
                &mut entries,
            )
            .await?;

            let duration_ms = start.elapsed().as_millis() as u64;

            // Format output
            let mut output = format!("Directory: {}\n\n", resolved_path.display());

            if entries.is_empty() {
                output.push_str("(empty directory)");
            } else {
                for entry in &entries {
                    output.push_str(entry);
                    output.push('\n');
                }
                output.push_str(&format!("\n{} entries", entries.len()));
            }

            Ok(ToolOutput::success(call_id, output, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        false // Reading directory is safe
    }
}

/// Recursively list directory contents
async fn list_directory_recursive(
    base_path: &std::path::Path,
    current_path: &std::path::Path,
    show_hidden: bool,
    recursive: bool,
    max_depth: usize,
    current_depth: usize,
    entries: &mut Vec<String>,
) -> Result<(), mms_common::AgentError> {
    let mut read_dir = tokio::fs::read_dir(current_path)
        .await
        .map_err(|e| mms_common::AgentError::Io { source: e })?;

    let mut dir_entries = Vec::new();

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| mms_common::AgentError::Io { source: e })?
    {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Skip hidden files if not requested
        if !show_hidden && file_name_str.starts_with('.') {
            continue;
        }

        dir_entries.push(entry);
    }

    // Sort entries: directories first, then by name
    dir_entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();

        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    for entry in dir_entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        let indent = "  ".repeat(current_depth);
        let metadata = entry.metadata().await.ok();

        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        if is_dir {
            entries.push(format!("{}{}/", indent, file_name_str));

            // Recurse if requested and within depth limit
            if recursive && current_depth < max_depth {
                Box::pin(list_directory_recursive(
                    base_path,
                    &path,
                    show_hidden,
                    recursive,
                    max_depth,
                    current_depth + 1,
                    entries,
                ))
                .await?;
            }
        } else {
            let size_str = format_size(size);
            entries.push(format!("{}{}  ({})", indent, file_name_str, size_str));
        }
    }

    Ok(())
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_context(temp_dir: &TempDir) -> ToolContext {
        let config = mms_config::Config::default();
        let mut ctx = ToolContext::new(Arc::new(config));
        ctx.set_cwd(temp_dir.path().to_path_buf());
        ctx
    }

    #[tokio::test]
    async fn test_list_directory_basic() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = ListDirectoryHandler;

        let call = ToolCall::new("test-1", "list_directory", json!({}));

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("file1.txt"));
        assert!(result.content.contains("file2.txt"));
        assert!(result.content.contains("subdir/"));
    }

    #[tokio::test]
    async fn test_list_directory_hidden() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("visible.txt"), "content").unwrap();
        std::fs::write(temp_dir.path().join(".hidden"), "content").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = ListDirectoryHandler;

        // Without show_hidden
        let call = ToolCall::new(
            "test-1",
            "list_directory",
            json!({ "show_hidden": false }),
        );
        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.content.contains("visible.txt"));
        assert!(!result.content.contains(".hidden"));

        // With show_hidden
        let call = ToolCall::new(
            "test-2",
            "list_directory",
            json!({ "show_hidden": true }),
        );
        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.content.contains("visible.txt"));
        assert!(result.content.contains(".hidden"));
    }

    #[tokio::test]
    async fn test_list_directory_recursive() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("subdir/nested.txt"), "content").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = ListDirectoryHandler;

        let call = ToolCall::new(
            "test-1",
            "list_directory",
            json!({ "recursive": true }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("subdir/"));
        assert!(result.content.contains("nested.txt"));
    }
}
