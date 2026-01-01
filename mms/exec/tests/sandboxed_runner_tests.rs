use mms_exec::{CommandRequest, SandboxedRunner};
use mms_execpolicy::{CommandPattern, Policy, PrefixRule};

fn cmd(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_command_request_builder() {
    let req = CommandRequest::new("ls")
        .with_args(vec!["-la".to_string()])
        .with_cwd("/tmp")
        .with_env("FOO", "bar")
        .with_timeout(5000);

    assert_eq!(req.command, "ls");
    assert_eq!(req.args, vec!["-la"]);
    assert_eq!(req.cwd, Some(std::path::PathBuf::from("/tmp")));
    assert_eq!(req.env, vec![("FOO".to_string(), "bar".to_string())]);
    assert_eq!(req.timeout_ms, Some(5000));
}

#[test]
fn test_command_request_as_vec() {
    let req = CommandRequest::new("git")
        .with_args(vec!["status".to_string(), "-s".to_string()]);

    assert_eq!(req.as_command_vec(), vec!["git", "status", "-s"]);
}

#[test]
fn test_sandboxed_runner_check_safe_command() {
    let runner = SandboxedRunner::default();
    assert!(runner.is_command_allowed(&cmd(&["ls"])));
    assert!(runner.is_command_allowed(&cmd(&["git", "status"])));
}

#[test]
fn test_sandboxed_runner_check_dangerous_command() {
    let runner = SandboxedRunner::default();
    assert!(runner.command_requires_approval(&cmd(&["rm", "-rf", "/"])));
    assert!(runner.command_requires_approval(&cmd(&["sudo", "rm"])));
}

#[test]
fn test_sandboxed_runner_with_policy() {
    let mut policy = Policy::new();
    policy.add_rule(PrefixRule::allow(CommandPattern::new("echo")));
    policy.add_rule(PrefixRule::forbid(CommandPattern::new("rm")));

    let runner = SandboxedRunner::default()
        .with_exec_policy(policy);

    assert!(runner.is_command_allowed(&cmd(&["echo", "hello"])));
    assert!(runner.is_command_forbidden(&cmd(&["rm", "file"])));
}

#[tokio::test]
async fn test_sandboxed_runner_forbidden_command_fails() {
    let mut policy = Policy::new();
    policy.add_rule(PrefixRule::forbid(CommandPattern::new("forbidden")));

    let runner = SandboxedRunner::default()
        .with_exec_policy(policy);

    let request = CommandRequest::new("forbidden");
    let result = runner.run(request).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_sandboxed_runner_unapproved_dangerous_fails() {
    let runner = SandboxedRunner::default();

    let request = CommandRequest::new("rm")
        .with_args(vec!["-rf".to_string()]);

    let result = runner.run_with_approval(request, false).await;

    assert!(result.is_err());
}
