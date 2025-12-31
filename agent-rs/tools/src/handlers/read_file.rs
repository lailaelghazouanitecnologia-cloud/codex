use serde_json::json;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

pub struct ReadFileHandler;

impl ToolHandler for ReadFileHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("read_file", "Read the contents of a file at the specified path")
            .with_parameters(
                json!({
                    "path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-based)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                }),
                vec!["path".into()],
            )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let path = call.get_string("path").unwrap_or_default();
        let offset = call.get_u64("offset").map(|v| v as usize);
        let limit = call.get_u64("limit").map(|v| v as usize);
        let resolved_path = ctx.resolve_path(&path);
        let call_id = call.id.clone();

        Box::pin(async move {
            let start = Instant::now();

            let content = tokio::fs::read_to_string(&resolved_path).await.map_err(|e| {
                agent_common::AgentError::Io { source: e }
            })?;

            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            let start_line = offset.unwrap_or(0);
            let end_line = limit.map(|l| (start_line + l).min(total_lines)).unwrap_or(total_lines);

            let selected_lines: Vec<String> = lines
                .iter()
                .enumerate()
                .skip(start_line)
                .take(end_line - start_line)
                .map(|(i, line)| format!("{:>6}\t{}", i + 1, line))
                .collect();

            let output = selected_lines.join("\n");
            let duration_ms = start.elapsed().as_millis() as u64;

            Ok(ToolOutput::success(call_id, output, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        false
    }
}
