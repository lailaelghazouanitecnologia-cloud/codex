use mms_execpolicy::{Evaluation, default_heuristics};
use mms_linux_sandbox::{apply_sandbox_policy, is_sandbox_supported};
use mms_shell::command_safety::{
    is_dangerous_to_exec, is_known_safe_command, get_danger_reason,
};
use serde_json::json;
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

pub struct ShellHandler;

impl ShellHandler {
    fn check_command(&self, ctx: &ToolContext, command: &[String]) -> Evaluation {
        ctx.exec_policy()
            .map(|p| p.check(command))
            .unwrap_or_else(|| {
                let decision = default_heuristics(command);
                Evaluation { decision, matched_rules: vec![] }
            })
    }

    fn parse_command_to_vec(command: &str) -> Vec<String> {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };
        vec![shell.to_string(), shell_arg.to_string(), command.to_string()]
    }

    /// Get detailed safety analysis for a command.
    pub fn analyze_command_safety(command: &str) -> CommandSafetyInfo {
        let command_vec = Self::parse_command_to_vec(command);
        let command_strs: Vec<&str> = command_vec.iter().map(|s| s.as_str()).collect();

        let is_safe = is_known_safe_command(&command_strs);
        let danger_pattern = is_dangerous_to_exec(&command_strs);
        let danger_reason = get_danger_reason(&command_strs);

        CommandSafetyInfo {
            is_safe,
            is_dangerous: danger_pattern.is_some(),
            danger_reason,
            severity: danger_pattern.map(|p| p.severity),
        }
    }
}

/// Detailed safety information for a command.
#[derive(Debug, Clone)]
pub struct CommandSafetyInfo {
    /// Whether the command is known to be safe.
    pub is_safe: bool,
    /// Whether the command has dangerous patterns.
    pub is_dangerous: bool,
    /// Reason why the command is dangerous (if applicable).
    pub danger_reason: Option<String>,
    /// Severity level (1-5) if dangerous.
    pub severity: Option<u8>,
}

impl CommandSafetyInfo {
    /// Check if the command needs approval.
    pub fn needs_approval(&self) -> bool {
        self.is_dangerous || !self.is_safe
    }

    /// Check if the command should be forbidden.
    pub fn is_forbidden(&self) -> bool {
        self.severity.map(|s| s >= 5).unwrap_or(false)
    }
}

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
        let sandbox_policy = ctx.sandbox_policy().clone();
        let sandbox_enabled = ctx.sandbox_enabled();

        // Check exec policy
        let command_vec = Self::parse_command_to_vec(&command);
        let eval = self.check_command(ctx, &command_vec);

        if eval.is_forbidden() {
            return Box::pin(async move {
                Ok(ToolOutput::error(
                    call_id,
                    format!("Command forbidden by policy: {}", command),
                    0,
                ))
            });
        }

        Box::pin(async move {
            let start = Instant::now();

            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

            // Determine if we should sandbox
            let should_sandbox = sandbox_enabled
                && is_sandbox_supported()
                && !sandbox_policy.has_full_disk_write_access();

            if should_sandbox {
                let sandbox = sandbox_policy.clone().add_writable_root(&cwd);
                if let Err(e) = apply_sandbox_policy(&sandbox, &cwd) {
                    tracing::warn!("Failed to apply sandbox policy: {:?}, continuing without sandbox", e);
                }
            }

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

                    let sandbox_status = if should_sandbox { " [sandboxed]" } else { "" };
                    let content = format!(
                        "Exit code: {}{}\n\nStdout:\n{}\n\nStderr:\n{}",
                        exit_code, sandbox_status, stdout, stderr
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

    fn is_dangerous(&self, call: &ToolCall) -> bool {
        // Check if command requires approval based on heuristics
        let command = call.get_string("command").unwrap_or_default();
        let command_vec = Self::parse_command_to_vec(&command);
        let eval_decision = default_heuristics(&command_vec);

        // Dangerous if requires prompt or is forbidden
        !eval_decision.is_allowed()
    }
}
