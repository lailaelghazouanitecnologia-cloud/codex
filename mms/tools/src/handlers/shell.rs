use serde_json::json;
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

pub struct ShellHandler;

impl ToolHandler for ShellHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("shell", "Execute a shell command")
            .with_parameters(
                json!({
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Timeout in milliseconds"
                    }
                }),
                vec!["command".into()],
            )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let command = call.get_string("command").unwrap_or_default();
        let timeout_ms = call.get_u64("timeout_ms").unwrap_or(ctx.timeout_ms());
        let cwd = ctx.cwd().clone();
        let call_id = call.id.clone();

        Box::pin(async move {
            let start = Instant::now();

            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

            let child = Command::new(shell)
                .arg(shell_arg)
                .arg(&command)
                .current_dir(&cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| mms_common::AgentError::Io { source: e })?;

            let timeout = tokio::time::Duration::from_millis(timeout_ms);
            let output_result = tokio::time::timeout(timeout, child.wait_with_output()).await;

            let duration_ms = start.elapsed().as_millis() as u64;

            match output_result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let exit_code = output.status.code().unwrap_or(-1);

                    let content = format!(
                        "Exit code: {}\n\nStdout:\n{}\n\nStderr:\n{}",
                        exit_code, stdout, stderr
                    );

                    if output.status.success() {
                        Ok(ToolOutput::success(call_id, content, duration_ms))
                    } else {
                        Ok(ToolOutput::error(call_id, content, duration_ms))
                    }
                }
                Ok(Err(e)) => {
                    Ok(ToolOutput::error(call_id, e.to_string(), duration_ms))
                }
                Err(_) => {
                    let message = format!("Command timed out after {}ms", timeout_ms);
                    Ok(ToolOutput::error(call_id, message, duration_ms))
                }
            }
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        true
    }
}
