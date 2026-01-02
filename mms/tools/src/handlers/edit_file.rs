//! Edit file handler for precise line-based modifications.
//!
//! This tool performs exact string replacement in files, similar to Claude Code's Edit tool.

use serde_json::json;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Handler for editing files with find/replace functionality.
///
/// This tool finds an exact string in a file and replaces it with a new string.
/// The old_string must be unique in the file for the edit to succeed.
pub struct EditFileHandler;

impl ToolHandler for EditFileHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "edit_file",
            "Perform exact string replacement in a file. The old_string must be unique in the file.",
        )
        .with_parameters(
            json!({
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace (must be unique in the file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The string to replace old_string with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences of old_string. Default is false.",
                    "default": false
                }
            }),
            vec!["path".into(), "old_string".into(), "new_string".into()],
        )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let path = call.get_string("path").unwrap_or_default();
        let old_string = call.get_string("old_string").unwrap_or_default();
        let new_string = call.get_string("new_string").unwrap_or_default();
        let replace_all = call.get_bool("replace_all").unwrap_or(false);
        let resolved_path = ctx.resolve_path(&path);
        let call_id = call.id.clone();

        Box::pin(async move {
            let start = Instant::now();

            // Read the file
            let content = tokio::fs::read_to_string(&resolved_path)
                .await
                .map_err(|e| mms_common::AgentError::Io { source: e })?;

            // Validate that old_string exists
            if !content.contains(&old_string) {
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ToolOutput::error(
                    call_id,
                    format!(
                        "The string to replace was not found in {}",
                        resolved_path.display()
                    ),
                    duration_ms,
                ));
            }

            // Check uniqueness if not replace_all
            if !replace_all {
                let occurrences = content.matches(&old_string).count();
                if occurrences > 1 {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(ToolOutput::error(
                        call_id,
                        format!(
                            "The string to replace occurs {} times in {}. Use replace_all=true or provide more context to make it unique.",
                            occurrences,
                            resolved_path.display()
                        ),
                        duration_ms,
                    ));
                }
            }

            // Perform the replacement
            let new_content = if replace_all {
                content.replace(&old_string, &new_string)
            } else {
                content.replacen(&old_string, &new_string, 1)
            };

            // Write back
            tokio::fs::write(&resolved_path, &new_content)
                .await
                .map_err(|e| mms_common::AgentError::Io { source: e })?;

            let duration_ms = start.elapsed().as_millis() as u64;
            let replacements = if replace_all {
                content.matches(&old_string).count()
            } else {
                1
            };

            let message = format!(
                "Edited {}: {} replacement(s) made",
                resolved_path.display(),
                replacements
            );

            Ok(ToolOutput::success(call_id, message, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        true // File modification requires approval
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
    async fn test_edit_file_simple_replace() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello World").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = EditFileHandler;

        let call = ToolCall::new(
            "test-1",
            "edit_file",
            json!({
                "path": "test.txt",
                "old_string": "World",
                "new_string": "Rust"
            }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello Rust");
    }

    #[tokio::test]
    async fn test_edit_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello World").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = EditFileHandler;

        let call = ToolCall::new(
            "test-1",
            "edit_file",
            json!({
                "path": "test.txt",
                "old_string": "NotFound",
                "new_string": "Replacement"
            }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(!result.success);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_edit_file_not_unique() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "foo bar foo baz").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = EditFileHandler;

        let call = ToolCall::new(
            "test-1",
            "edit_file",
            json!({
                "path": "test.txt",
                "old_string": "foo",
                "new_string": "qux"
            }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(!result.success);
        assert!(result.content.contains("2 times"));
    }

    #[tokio::test]
    async fn test_edit_file_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "foo bar foo baz").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = EditFileHandler;

        let call = ToolCall::new(
            "test-1",
            "edit_file",
            json!({
                "path": "test.txt",
                "old_string": "foo",
                "new_string": "qux",
                "replace_all": true
            }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("2 replacement"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "qux bar qux baz");
    }
}
