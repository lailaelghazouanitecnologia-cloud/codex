use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::policy::SandboxPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxContainer {
    pub id: String,
    pub state: ContainerState,
    pub policy: SandboxPolicy,
    pub working_dir: PathBuf,
    pub env_vars: std::collections::HashMap<String, String>,
}

impl SandboxContainer {
    pub fn new(id: impl Into<String>, policy: SandboxPolicy) -> Self {
        Self {
            id: id.into(),
            state: ContainerState::Created,
            policy,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            env_vars: std::collections::HashMap::new(),
        }
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = dir.into();
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    pub fn start(&mut self) -> Result<(), ContainerError> {
        match self.state {
            ContainerState::Created | ContainerState::Stopped => {
                self.state = ContainerState::Running;
                Ok(())
            }
            ContainerState::Running => Err(ContainerError::AlreadyRunning),
            ContainerState::Paused => {
                self.state = ContainerState::Running;
                Ok(())
            }
            ContainerState::Failed => Err(ContainerError::Failed),
        }
    }

    pub fn stop(&mut self) -> Result<(), ContainerError> {
        match self.state {
            ContainerState::Running | ContainerState::Paused => {
                self.state = ContainerState::Stopped;
                Ok(())
            }
            ContainerState::Stopped => Err(ContainerError::NotRunning),
            ContainerState::Created => Err(ContainerError::NotRunning),
            ContainerState::Failed => Err(ContainerError::Failed),
        }
    }

    pub fn pause(&mut self) -> Result<(), ContainerError> {
        match self.state {
            ContainerState::Running => {
                self.state = ContainerState::Paused;
                Ok(())
            }
            _ => Err(ContainerError::NotRunning),
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == ContainerState::Running
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self.state, ContainerState::Stopped | ContainerState::Created)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerError {
    AlreadyRunning,
    NotRunning,
    Failed,
    PolicyViolation(String),
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "Container is already running"),
            Self::NotRunning => write!(f, "Container is not running"),
            Self::Failed => write!(f, "Container is in failed state"),
            Self::PolicyViolation(msg) => write!(f, "Policy violation: {}", msg),
        }
    }
}

impl std::error::Error for ContainerError {}
