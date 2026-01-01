use mms_common::AgentResult;
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

pub struct SandboxExecutor {
    containers: HashMap<String, SandboxContainer>,
}

impl SandboxExecutor {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
        }
    }

    pub fn create_container(&mut self, container: SandboxContainer) -> &SandboxContainer {
        let id = container.id.clone();
        self.containers.insert(id.clone(), container);
        self.containers.get(&id).unwrap()
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
}

impl Default for SandboxExecutor {
    fn default() -> Self {
        Self::new()
    }
}
