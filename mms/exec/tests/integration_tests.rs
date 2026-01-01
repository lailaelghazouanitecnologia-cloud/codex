use mms_exec::SandboxedRunner;
use mms_execpolicy::parse_policy;
use mms_linux_sandbox::SandboxPolicy;
use std::path::PathBuf;

fn test_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"))
}

// Integration tests for SandboxedRunner with Policy

#[tokio::test]
async fn test_runner_with_policy_allows_safe_command() {
    let policy_content = r#"
[[rules]]
type = "glob"
program = "echo"
decision = "allow"
"#;
    let policy = parse_policy(policy_content).unwrap();

    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy)
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = mms_exec::CommandRequest::new("echo")
        .with_args(vec!["hello".into()]);

    let result = runner.run(request).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.success());
    assert!(output.stdout.contains("hello"));
}

#[tokio::test]
async fn test_runner_with_policy_forbids_dangerous_command() {
    let policy_content = r#"
[[rules]]
type = "prefix"
command = ["rm", "-rf"]
decision = "forbidden"
"#;
    let policy = parse_policy(policy_content).unwrap();

    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy);

    let request = mms_exec::CommandRequest::new("rm")
        .with_args(vec!["-rf".into(), "/tmp/nonexistent".into()]);

    let result = runner.run(request).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("forbidden"));
}

#[tokio::test]
async fn test_runner_with_policy_prompts_unknown_command() {
    let policy_content = r#"
[policy]
default = "prompt"
"#;
    let policy = parse_policy(policy_content).unwrap();

    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy);

    // Check that unknown command requires approval
    let cmd = vec!["some-unknown-program".to_string()];
    assert!(runner.command_requires_approval(&cmd));
}

#[tokio::test]
async fn test_runner_unapproved_prompt_command_fails() {
    let policy_content = r#"
[policy]
default = "prompt"
"#;
    let policy = parse_policy(policy_content).unwrap();

    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy);

    let request = mms_exec::CommandRequest::new("some-unknown-command");

    // Run without approval - should fail
    let result = runner.run_with_approval(request, false).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("requires approval"));
}

#[tokio::test]
async fn test_runner_approved_prompt_command_executes() {
    let policy_content = r#"
[policy]
default = "prompt"

[[rules]]
type = "glob"
program = "echo"
decision = "prompt"
"#;
    let policy = parse_policy(policy_content).unwrap();

    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy)
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = mms_exec::CommandRequest::new("echo")
        .with_args(vec!["approved".into()]);

    // Run with approval - should succeed
    let result = runner.run_with_approval(request, true).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.success());
}

#[tokio::test]
async fn test_runner_with_timeout() {
    let runner = SandboxedRunner::new(test_cwd())
        .with_default_timeout(100) // 100ms timeout
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = mms_exec::CommandRequest::new("sleep")
        .with_args(vec!["10".into()]); // 10 seconds - will timeout

    let result = runner.run(request).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("timed out"));
}

#[tokio::test]
async fn test_runner_with_env() {
    let runner = SandboxedRunner::new(test_cwd())
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = mms_exec::CommandRequest::new("sh")
        .with_args(vec!["-c".into(), "echo $TEST_VAR".into()])
        .with_env("TEST_VAR", "hello_world");

    let result = runner.run(request).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.stdout.contains("hello_world"));
}

#[tokio::test]
async fn test_runner_with_cwd() {
    let runner = SandboxedRunner::new(test_cwd())
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = mms_exec::CommandRequest::new("pwd")
        .with_cwd("/tmp");

    let result = runner.run(request).await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.stdout.contains("/tmp"));
}

#[tokio::test]
async fn test_runner_policy_precedence() {
    // Test that more specific rules take precedence
    let policy_content = r#"
[policy]
default = "forbidden"

[[rules]]
type = "glob"
program = "ls"
decision = "allow"
"#;
    let policy = parse_policy(policy_content).unwrap();

    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy)
        .with_sandbox_policy(SandboxPolicy::permissive());

    // ls should be allowed even with forbidden default
    let request = mms_exec::CommandRequest::new("ls");
    let result = runner.run(request).await;
    assert!(result.is_ok());

    // Other commands should be forbidden
    let cmd = vec!["cat".to_string()];
    assert!(runner.is_command_forbidden(&cmd));
}

#[test]
fn test_sandbox_policy_integration_with_config() {
    use mms_linux_sandbox::{NetworkAccess, DiskAccess};

    // Test that SandboxPolicy can be configured correctly
    let policy = SandboxPolicy::default()
        .with_network(NetworkAccess::None)
        .with_disk_read(DiskAccess::Full)
        .with_disk_write(DiskAccess::Restricted)
        .add_writable_root("/tmp")
        .add_readable_root("/home");

    // Verify the policy is configured
    assert!(!policy.has_full_disk_write_access());
}

#[test]
fn test_policy_check_methods() {
    let policy_content = r#"
[[rules]]
type = "glob"
program = "git"
decision = "allow"

[[rules]]
type = "prefix"
command = ["rm", "-rf"]
decision = "forbidden"
"#;
    let policy = parse_policy(policy_content).unwrap();
    let runner = SandboxedRunner::new(test_cwd())
        .with_exec_policy(policy);

    // Test is_command_allowed
    let git_cmd = vec!["git".to_string(), "status".to_string()];
    assert!(runner.is_command_allowed(&git_cmd));

    // Test is_command_forbidden
    let rm_cmd = vec!["rm".to_string(), "-rf".to_string(), "/".to_string()];
    assert!(runner.is_command_forbidden(&rm_cmd));

    // Test command_requires_approval
    let unknown_cmd = vec!["unknown".to_string()];
    assert!(runner.command_requires_approval(&unknown_cmd));
}
