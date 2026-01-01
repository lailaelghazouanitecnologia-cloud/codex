use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccess {
    None,
    UnixOnly,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskAccess {
    None,
    ReadOnly,
    Restricted,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritableRoot {
    pub root: PathBuf,
    #[serde(default)]
    pub recursive: bool,
}

impl WritableRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            recursive: true,
        }
    }

    pub fn non_recursive(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            recursive: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub network_access: NetworkAccess,
    pub disk_read_access: DiskAccess,
    pub disk_write_access: DiskAccess,
    #[serde(default)]
    pub writable_roots: Vec<WritableRoot>,
    #[serde(default)]
    pub readable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub allow_ptrace: bool,
    #[serde(default)]
    pub allow_new_privs: bool,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            network_access: NetworkAccess::None,
            disk_read_access: DiskAccess::Full,
            disk_write_access: DiskAccess::Restricted,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            allow_ptrace: false,
            allow_new_privs: false,
        }
    }
}

impl SandboxPolicy {
    pub fn permissive() -> Self {
        Self {
            network_access: NetworkAccess::Full,
            disk_read_access: DiskAccess::Full,
            disk_write_access: DiskAccess::Full,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            allow_ptrace: true,
            allow_new_privs: true,
        }
    }

    pub fn restrictive() -> Self {
        Self {
            network_access: NetworkAccess::None,
            disk_read_access: DiskAccess::ReadOnly,
            disk_write_access: DiskAccess::None,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            allow_ptrace: false,
            allow_new_privs: false,
        }
    }

    pub fn with_network(mut self, access: NetworkAccess) -> Self {
        self.network_access = access;
        self
    }

    pub fn with_disk_read(mut self, access: DiskAccess) -> Self {
        self.disk_read_access = access;
        self
    }

    pub fn with_disk_write(mut self, access: DiskAccess) -> Self {
        self.disk_write_access = access;
        self
    }

    pub fn add_writable_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.writable_roots.push(WritableRoot::new(root));
        self
    }

    pub fn add_readable_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.readable_roots.push(root.into());
        self
    }

    pub fn has_full_network_access(&self) -> bool {
        matches!(self.network_access, NetworkAccess::Full)
    }

    pub fn has_full_disk_write_access(&self) -> bool {
        matches!(self.disk_write_access, DiskAccess::Full)
    }

    pub fn has_full_disk_read_access(&self) -> bool {
        matches!(self.disk_read_access, DiskAccess::Full)
    }

    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        let mut roots = self.writable_roots.clone();

        if matches!(self.disk_write_access, DiskAccess::Restricted) {
            roots.push(WritableRoot::new(cwd));
        }

        roots
    }

    pub fn get_readable_roots_with_cwd(&self, cwd: &Path) -> Vec<PathBuf> {
        let mut roots = self.readable_roots.clone();

        if matches!(self.disk_read_access, DiskAccess::Restricted) {
            roots.push(cwd.to_path_buf());
        }

        roots
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub policy: SandboxPolicy,
    pub cwd: PathBuf,
    #[serde(default)]
    pub env_vars: Vec<(String, String)>,
    #[serde(default)]
    pub inherit_env: bool,
}

impl SandboxConfig {
    pub fn new(policy: SandboxPolicy, cwd: impl Into<PathBuf>) -> Self {
        Self {
            policy,
            cwd: cwd.into(),
            env_vars: Vec::new(),
            inherit_env: true,
        }
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), value.into()));
        self
    }

    pub fn without_inherit_env(mut self) -> Self {
        self.inherit_env = false;
        self
    }
}
