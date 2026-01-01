use mms_common::AgentResult;
use mms_execpolicy::{Evaluation, Policy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Output;
use tokio::process::Command;

use crate::container::{ContainerError, SandboxContainer};
use crate::policy::Permission;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

impl ExecutionResult {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCheck {
    pub command: Vec<String>,
    pub evaluation: Evaluation,
    pub allowed: bool,
    pub requires_approval: bool,
}

pub struct SandboxExecutor {
    containers: HashMap<String, SandboxContainer>,
    exec_policy: Option<Policy>,
}

impl SandboxExecutor {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
            exec_policy: None,
        }
    }

    pub fn with_exec_policy(mut self, policy: Policy) -> Self {
        self.exec_policy = Some(policy);
        self
    }

    pub fn set_exec_policy(&mut self, policy: Policy) {
        self.exec_policy = Some(policy);
    }

    pub fn check_command(&self, command: &[String]) -> CommandCheck {
        let evaluation = self.exec_policy
            .as_ref()
            .map(|p| p.check(command))
            .unwrap_or_else(|| {
                let decision = mms_execpolicy::default_heuristics(command);
                Evaluation { decision, matched_rules: vec![] }
            });

        CommandCheck {
            command: command.to_vec(),
            evaluation: evaluation.clone(),
            allowed: evaluation.is_allowed(),
            requires_approval: evaluation.requires_prompt(),
        }
    }

    pub fn check_commands<'a>(&self, commands: impl IntoIterator<Item = &'a [String]>) -> Vec<CommandCheck> {
        commands.into_iter().map(|cmd| self.check_command(cmd)).collect()
    }

    pub fn create_container(&mut self, container: SandboxContainer) -> &SandboxContainer {
        let id = container.id.clone();
        self.containers.insert(id.clone(), container);
        self.containers.get(&id).ok_or("Container not found").unwrap()
    }

    pub fn get_container(&self, id: &str) -> Option<&SandboxContainer> {
        self.containers.get(id)
    }

    pub fn remove_container(&mut self, id: &str) -> Option<SandboxContainer> {
        self.containers.remove(id)
    }

    pub async fn execute(
        &mut self,
        container_id: &str,
        command: &str,
        args: &[&str],
    ) -> AgentResult<ExecutionResult> {
        {
            let container = self
                .containers
                .get_mut(container_id)
                .ok_or_else(|| mms_common::AgentError::not_found(format!("Container not found: {}", container_id)))?;

            if !container.is_running() {
                container.start().map_err(|e| mms_common::AgentError::execution(e.to_string()))?;
            }
        }

        let container = self.containers.get(container_id).ok_or_else(|| {
            mms_common::AgentError::not_found(format!("Container not found: {}", container_id))
        })?;

        Self::validate_execution(container, command)?;

        let full_command: Vec<String> = std::iter::once(command.to_string())
            .chain(args.iter().map(|s| s.to_string()))
            .collect();

        self.validate_exec_policy(&full_command)?;

        let start = std::time::Instant::now();

        let output = Self::run_command(container, command, args)
            .await
            .map_err(|e| mms_common::AgentError::execution(e.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
        })
    }

    pub async fn execute_checked(
        &mut self,
        container_id: &str,
        command: &str,
        args: &[&str],
        approved: bool,
    ) -> AgentResult<ExecutionResult> {
        let full_command: Vec<String> = std::iter::once(command.to_string())
            .chain(args.iter().map(|s| s.to_string()))
            .collect();

        let check = self.check_command(&full_command);

        if check.evaluation.is_forbidden() {
            return Err(mms_common::AgentError::execution(
                format!("Command forbidden by policy: {}", command)
            ));
        }

        if check.requires_approval && !approved {
            return Err(mms_common::AgentError::execution(
                format!("Command requires approval: {}", command)
            ));
        }

        self.execute(container_id, command, args).await
    }

    fn validate_exec_policy(&self, command: &[String]) -> AgentResult<()> {
        if let Some(policy) = &self.exec_policy {
            let eval = policy.check(command);
            if eval.is_forbidden() {
                return Err(mms_common::AgentError::execution(
                    ContainerError::PolicyViolation(
                        format!("Command forbidden by exec policy: {:?}", command)
                    ).to_string()
                ));
            }
        }
        Ok(())
    }

    fn validate_execution(container: &SandboxContainer, command: &str) -> AgentResult<()> {
        if !container.policy.has_permission(Permission::Execute) {
            return Err(mms_common::AgentError::execution(
                ContainerError::PolicyViolation("Execute permission denied".to_string()).to_string(),
            ));
        }

        if !container.policy.is_command_allowed(command) {
            return Err(mms_common::AgentError::execution(
                ContainerError::PolicyViolation(format!("Command not allowed: {}", command)).to_string(),
            ));
        }

        Ok(())
    }

    async fn run_command(
        container: &SandboxContainer,
        command: &str,
        args: &[&str],
    ) -> std::io::Result<Output> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.current_dir(&container.working_dir);

        for (key, value) in &container.env_vars {
            cmd.env(key, value);
        }

        let timeout = std::time::Duration::from_millis(container.policy.max_execution_time_ms());

        tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Command timed out"))?
    }

    pub fn container_count(&self) -> usize {
        self.containers.len()
    }

    pub fn has_exec_policy(&self) -> bool {
        self.exec_policy.is_some()
    }
}

impl Default for SandboxExecutor {
    fn default() -> Self {
        Self::new()
    }
}
