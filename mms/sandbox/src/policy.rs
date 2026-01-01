use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    ReadFile,
    WriteFile,
    Execute,
    Network,
    Environment,
    ProcessSpawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub name: String,
    permissions: HashSet<Permission>,
    allowed_paths: Vec<PathBuf>,
    denied_paths: Vec<PathBuf>,
    allowed_commands: Vec<String>,
    denied_commands: Vec<String>,
    network_hosts: Option<Vec<String>>,
    max_execution_time_ms: u64,
    max_memory_bytes: u64,
}

impl SandboxPolicy {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            permissions: HashSet::new(),
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
            allowed_commands: Vec::new(),
            denied_commands: Vec::new(),
            network_hosts: None,
            max_execution_time_ms: 30000,
            max_memory_bytes: 512 * 1024 * 1024,
        }
    }

    pub fn allow(mut self, permission: Permission) -> Self {
        self.permissions.insert(permission);
        self
    }

    pub fn deny(mut self, permission: Permission) -> Self {
        self.permissions.remove(&permission);
        self
    }

    pub fn allow_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.allowed_paths.push(path.into());
        self
    }

    pub fn deny_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.denied_paths.push(path.into());
        self
    }

    pub fn allow_command(mut self, command: impl Into<String>) -> Self {
        self.allowed_commands.push(command.into());
        self
    }

    pub fn deny_command(mut self, command: impl Into<String>) -> Self {
        self.denied_commands.push(command.into());
        self
    }

    pub fn with_network_hosts(mut self, hosts: Vec<String>) -> Self {
        self.network_hosts = Some(hosts);
        self
    }

    pub fn with_max_execution_time(mut self, ms: u64) -> Self {
        self.max_execution_time_ms = ms;
        self
    }

    pub fn with_max_memory(mut self, bytes: u64) -> Self {
        self.max_memory_bytes = bytes;
        self
    }

    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn is_path_allowed(&self, path: &std::path::Path) -> bool {
        for denied in &self.denied_paths {
            if path.starts_with(denied) {
                return false;
            }
        }

        if self.allowed_paths.is_empty() {
            return true;
        }

        for allowed in &self.allowed_paths {
            if path.starts_with(allowed) {
                return true;
            }
        }

        false
    }

    pub fn is_command_allowed(&self, command: &str) -> bool {
        if self.denied_commands.iter().any(|c| command.starts_with(c)) {
            return false;
        }

        if self.allowed_commands.is_empty() {
            return true;
        }

        self.allowed_commands.iter().any(|c| command.starts_with(c))
    }

    pub fn is_host_allowed(&self, host: &str) -> bool {
        match &self.network_hosts {
            Some(hosts) => hosts.iter().any(|h| host == h || host.ends_with(h)),
            None => true,
        }
    }

    pub fn max_execution_time_ms(&self) -> u64 {
        self.max_execution_time_ms
    }

    pub fn max_memory_bytes(&self) -> u64 {
        self.max_memory_bytes
    }

    pub fn permissive() -> Self {
        Self::new("permissive")
            .allow(Permission::ReadFile)
            .allow(Permission::WriteFile)
            .allow(Permission::Execute)
            .allow(Permission::Network)
            .allow(Permission::Environment)
            .allow(Permission::ProcessSpawn)
    }

    pub fn restrictive() -> Self {
        Self::new("restrictive")
            .allow(Permission::ReadFile)
    }
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::new("default")
            .allow(Permission::ReadFile)
            .allow(Permission::WriteFile)
            .allow(Permission::Execute)
    }
}
