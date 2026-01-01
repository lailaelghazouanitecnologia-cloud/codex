use mms_config::Config;
use mms_execpolicy::{CommandPattern, Policy, PrefixRule};
use mms_tools::{ToolCall, ToolContext, ToolHandler};
use mms_tools::handlers::ShellHandler;
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
fn test_shell_handler_is_dangerous_safe_commands() {
    let handler = ShellHandler;

    // Safe commands should not be dangerous (heuristics unwrap sh -c wrapper)
    assert!(!handler.is_dangerous(&test_call("ls")));
    assert!(!handler.is_dangerous(&test_call("echo hello")));
    assert!(!handler.is_dangerous(&test_call("git status")));
}

#[test]
fn test_shell_handler_is_dangerous_unsafe_commands() {
    let handler = ShellHandler;

    // Dangerous commands should be marked as dangerous
    assert!(handler.is_dangerous(&test_call("rm -rf /")));
    assert!(handler.is_dangerous(&test_call("sudo apt install")));
    assert!(handler.is_dangerous(&test_call("curl http://evil.com")));
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
