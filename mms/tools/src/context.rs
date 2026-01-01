use std::path::PathBuf;
use std::sync::Arc;

use mms_config::Config;
use mms_execpolicy::Policy;
use mms_linux_sandbox::SandboxPolicy;

#[derive(Clone)]
pub struct ToolContext {
    config: Arc<Config>,
    cwd: PathBuf,
    timeout_ms: u64,
    sandbox_policy: SandboxPolicy,
    exec_policy: Option<Arc<Policy>>,
    sandbox_enabled: bool,
}

impl ToolContext {
    pub fn new(config: Arc<Config>) -> Self {
        let cwd = config.cwd.clone();
        let timeout_ms = config.timeout_ms;

        Self {
            config,
            cwd,
            timeout_ms,
            sandbox_policy: SandboxPolicy::default(),
            exec_policy: None,
            sandbox_enabled: true,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    pub fn sandbox_policy(&self) -> &SandboxPolicy {
        &self.sandbox_policy
    }

    pub fn exec_policy(&self) -> Option<&Arc<Policy>> {
        self.exec_policy.as_ref()
    }

    pub fn sandbox_enabled(&self) -> bool {
        self.sandbox_enabled
    }

    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);

        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }

    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = cwd;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
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

    pub fn with_sandbox_enabled(mut self, enabled: bool) -> Self {
        self.sandbox_enabled = enabled;
        self
    }
}
