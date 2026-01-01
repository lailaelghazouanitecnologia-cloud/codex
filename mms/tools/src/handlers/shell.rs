use mms_execpolicy::{Evaluation, default_heuristics};
use mms_linux_sandbox::{apply_sandbox_policy, is_sandbox_supported};
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

#[cfg(test)]
mod tests {
    use super::*;
    use mms_config::Config;
    use mms_execpolicy::{CommandPattern, Policy, PrefixRule};
    use std::sync::Arc;

    fn test_context() -> ToolContext {
        let config = Arc::new(Config::default());
        ToolContext::new(config)
    }

    fn test_call(command: &str) -> ToolCall {
        ToolCall {
            id: "test-id".into(),
            name: "shell".into(),
            arguments: serde_json::json!({
                "command": command
            }),
        }
    }

    #[test]
    fn test_shell_handler_spec() {
        let handler = ShellHandler;
        let spec = handler.spec();
        assert_eq!(spec.name, "shell");
    }

    #[test]
    fn test_shell_handler_is_dangerous_default() {
        let handler = ShellHandler;

        // All shell commands are considered dangerous by default heuristics
        // because they're wrapped in "sh -c" which is not in the safe list
        assert!(handler.is_dangerous(&test_call("ls")));
        assert!(handler.is_dangerous(&test_call("rm -rf /")));
    }

    #[test]
    fn test_shell_handler_parse_command() {
        let vec = ShellHandler::parse_command_to_vec("ls -la");
        assert_eq!(vec[0], "sh");
        assert_eq!(vec[1], "-c");
        assert_eq!(vec[2], "ls -la");
    }

    #[test]
    fn test_shell_handler_check_command_with_policy() {
        let handler = ShellHandler;

        // Create a policy that allows "sh" commands
        let mut policy = Policy::new();
        policy.add_rule(PrefixRule::allow(CommandPattern::new("sh")));
        policy.add_rule(PrefixRule::forbid(
            CommandPattern::new("sh")
                .with_single_arg("-c")
                .with_single_arg("rm")
        ));

        let ctx = test_context().with_exec_policy(policy);

        // Check allowed command (sh -c ls is allowed)
        let command_vec = ShellHandler::parse_command_to_vec("ls");
        let eval = handler.check_command(&ctx, &command_vec);
        assert!(eval.is_allowed());
    }

    #[test]
    fn test_shell_handler_check_command_default_heuristics() {
        let handler = ShellHandler;
        let ctx = test_context();

        // All commands wrapped in sh -c require prompt by default heuristics
        // because "sh" is not in the safe program list
        let command_vec = ShellHandler::parse_command_to_vec("ls");
        let eval = handler.check_command(&ctx, &command_vec);
        assert!(eval.requires_prompt());

        let command_vec = ShellHandler::parse_command_to_vec("rm -rf /");
        let eval = handler.check_command(&ctx, &command_vec);
        assert!(eval.requires_prompt());
    }

    #[tokio::test]
    async fn test_shell_handler_forbidden_command_returns_error() {
        let handler = ShellHandler;

        let mut policy = Policy::new();
        policy.add_rule(PrefixRule::forbid(CommandPattern::new("sh")));

        let ctx = test_context().with_exec_policy(policy);
        let call = test_call("dangerous command");

        let future = handler.execute(&ctx, call);
        let result = future.await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
        assert!(output.content.contains("forbidden"));
    }

    #[tokio::test]
    async fn test_shell_handler_allowed_command_executes() {
        let handler = ShellHandler;

        // Allow all sh commands
        let mut policy = Policy::new();
        policy.add_rule(PrefixRule::allow(CommandPattern::new("sh")));

        let ctx = test_context().with_exec_policy(policy);
        let call = test_call("echo hello");

        let future = handler.execute(&ctx, call);
        let result = future.await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.content.contains("hello"));
    }
}
