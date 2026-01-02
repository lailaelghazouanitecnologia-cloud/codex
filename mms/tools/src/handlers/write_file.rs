use serde_json::json;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Handler for writing files.
///
/// This tool creates or overwrites files at the specified path.
/// If the parent directory doesn't exist, it will be created.
///
/// When a turn diff tracker is configured, all file operations
/// are recorded for potential undo operations.
pub struct WriteFileHandler;

impl ToolHandler for WriteFileHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("write_file", "Write content to a file at the specified path")
            .with_parameters(
                json!({
                    "path": {
                        "type": "string",
                        "description": "The path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                }),
                vec!["path".into(), "content".into()],
            )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let path = call.get_string("path").unwrap_or_default();
        let content = call.get_string("content").unwrap_or_default();
        let resolved_path = ctx.resolve_path(&path);
        let call_id = call.id.clone();
        let ctx = ctx.clone();

        Box::pin(async move {
            let start = Instant::now();

            // Check if file exists to determine if this is a creation or modification
            let file_exists = resolved_path.exists();
            let original_content = if file_exists {
                // Read original content for undo functionality
                tokio::fs::read_to_string(&resolved_path).await.ok()
            } else {
                None
            };

            // Create parent directory if needed
            if let Some(parent) = resolved_path.parent() {
                if !parent.exists() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| {
                        mms_common::AgentError::Io { source: e }
                    })?;
                }
            }

            // Write the file
            tokio::fs::write(&resolved_path, &content).await.map_err(|e| {
                mms_common::AgentError::Io { source: e }
            })?;

            // Record the change in the diff tracker
            if file_exists {
                ctx.record_file_modification(
                    &resolved_path,
                    original_content,
                    Some(&call_id),
                ).await;
            } else {
                ctx.record_file_creation(&resolved_path, Some(&call_id)).await;
            }

            let bytes_written = content.len();
            let duration_ms = start.elapsed().as_millis() as u64;
            let action = if file_exists { "Updated" } else { "Created" };
            let message = format!("{} {} ({} bytes)", action, resolved_path.display(), bytes_written);

            Ok(ToolOutput::success(call_id, message, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        true
    }
}
