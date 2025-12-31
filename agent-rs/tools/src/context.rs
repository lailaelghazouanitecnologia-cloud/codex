use std::path::PathBuf;
use std::sync::Arc;

use agent_config::Config;

pub struct ToolContext {
    config: Arc<Config>,
    cwd: PathBuf,
    timeout_ms: u64,
}

impl ToolContext {
    pub fn new(config: Arc<Config>) -> Self {
        let cwd = config.cwd.clone();
        let timeout_ms = config.timeout_ms;

        Self {
            config,
            cwd,
            timeout_ms,
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
}
