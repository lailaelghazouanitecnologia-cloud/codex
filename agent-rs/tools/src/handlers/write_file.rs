use serde_json::json;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

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

        Box::pin(async move {
            let start = Instant::now();

            if let Some(parent) = resolved_path.parent() {
                if !parent.exists() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| {
                        agent_common::AgentError::Io { source: e }
                    })?;
                }
            }

            tokio::fs::write(&resolved_path, &content).await.map_err(|e| {
                agent_common::AgentError::Io { source: e }
            })?;

            let bytes_written = content.len();
            let duration_ms = start.elapsed().as_millis() as u64;
            let message = format!("Wrote {} bytes to {}", bytes_written, resolved_path.display());

            Ok(ToolOutput::success(call_id, message, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        true
    }
}
