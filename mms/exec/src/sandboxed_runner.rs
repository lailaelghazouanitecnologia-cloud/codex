use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use mms_common::{AgentError, AgentResult};
use mms_execpolicy::{Evaluation, Policy, default_heuristics};
use mms_linux_sandbox::{SandboxPolicy, apply_sandbox_policy, is_sandbox_supported};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub sandboxed: bool,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub timeout_ms: Option<u64>,
}

impl CommandRequest {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            timeout_ms: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn as_command_vec(&self) -> Vec<String> {
        let mut cmd = vec![self.command.clone()];
        cmd.extend(self.args.clone());
        cmd
    }
}

pub struct SandboxedRunner {
    sandbox_policy: SandboxPolicy,
    exec_policy: Option<Arc<Policy>>,
    default_cwd: PathBuf,
    default_timeout_ms: u64,
}

impl SandboxedRunner {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            sandbox_policy: SandboxPolicy::default(),
            exec_policy: None,
            default_cwd: cwd.into(),
            default_timeout_ms: 30000,
        }
    }

    pub fn with_sandbox_policy(mut self, policy: SandboxPolicy) -> Self {
        self.sandbox_policy = policy;
        self
    }

    pub fn with_exec_policy(mut self, policy: Policy) -> Self {
        self.exec_policy = Some(Arc::new(policy));
        self
    }

    pub fn with_exec_policy_arc(mut self, policy: Arc<Policy>) -> Self {
        self.exec_policy = Some(policy);
        self
    }

    pub fn with_default_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    pub fn check_command(&self, command: &[String]) -> Evaluation {
        self.exec_policy
            .as_ref()
            .map(|p| p.check(command))
            .unwrap_or_else(|| {
                let decision = default_heuristics(command);
                Evaluation { decision, matched_rules: vec![] }
            })
    }

    pub fn is_command_allowed(&self, command: &[String]) -> bool {
        let eval = self.check_command(command);
        eval.is_allowed()
    }

    pub fn command_requires_approval(&self, command: &[String]) -> bool {
        let eval = self.check_command(command);
        eval.requires_prompt()
    }

    pub fn is_command_forbidden(&self, command: &[String]) -> bool {
        let eval = self.check_command(command);
        eval.is_forbidden()
    }

    pub async fn run(&self, request: CommandRequest) -> AgentResult<CommandOutput> {
        self.run_with_approval(request, true).await
    }

    pub async fn run_with_approval(
        &self,
        request: CommandRequest,
        approved: bool,
    ) -> AgentResult<CommandOutput> {
        let command_vec = request.as_command_vec();
        let eval = self.check_command(&command_vec);

        if eval.is_forbidden() {
            return Err(AgentError::execution(
                format!("Command forbidden by policy: {}", request.command)
            ));
        }

        if eval.requires_prompt() && !approved {
            return Err(AgentError::execution(
                format!("Command requires approval: {}", request.command)
            ));
        }

        self.execute_command(request).await
    }

    async fn execute_command(&self, request: CommandRequest) -> AgentResult<CommandOutput> {
        let cwd = request.cwd.clone().unwrap_or_else(|| self.default_cwd.clone());
        let timeout_ms = request.timeout_ms.unwrap_or(self.default_timeout_ms);

        let sandboxed = is_sandbox_supported() && !self.sandbox_policy.has_full_disk_write_access();

        if sandboxed {
            self.execute_sandboxed(request, &cwd, timeout_ms).await
        } else {
            self.execute_direct(request, &cwd, timeout_ms).await
        }
    }

    async fn execute_direct(
        &self,
        request: CommandRequest,
        cwd: &Path,
        timeout_ms: u64,
    ) -> AgentResult<CommandOutput> {
        let start = std::time::Instant::now();

        let mut cmd = Command::new(&request.command);
        cmd.args(&request.args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &request.env {
            cmd.env(key, value);
        }

        let timeout = std::time::Duration::from_millis(timeout_ms);

        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| AgentError::execution("Command timed out"))?
            .map_err(|e| AgentError::execution(format!("Failed to execute command: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(CommandOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
            sandboxed: false,
        })
    }

    async fn execute_sandboxed(
        &self,
        request: CommandRequest,
        cwd: &Path,
        timeout_ms: u64,
    ) -> AgentResult<CommandOutput> {
        let sandbox_policy = self.sandbox_policy.clone()
            .add_writable_root(cwd);

        if let Err(e) = apply_sandbox_policy(&sandbox_policy, cwd) {
            tracing::warn!("Failed to apply sandbox policy: {:?}, falling back to direct execution", e);
            return self.execute_direct(request, cwd, timeout_ms).await;
        }

        let start = std::time::Instant::now();

        let mut cmd = Command::new(&request.command);
        cmd.args(&request.args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &request.env {
            cmd.env(key, value);
        }

        let timeout = std::time::Duration::from_millis(timeout_ms);

        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| AgentError::execution("Command timed out"))?
            .map_err(|e| AgentError::execution(format!("Failed to execute command: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(CommandOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
            sandboxed: true,
        })
    }

    pub fn sandbox_supported(&self) -> bool {
        is_sandbox_supported()
    }
}

impl Default for SandboxedRunner {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(req.cwd, Some(PathBuf::from("/tmp")));
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
        use mms_execpolicy::{CommandPattern, PrefixRule};

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
        use mms_execpolicy::{CommandPattern, PrefixRule};

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
}
