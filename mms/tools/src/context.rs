use std::path::PathBuf;
use std::sync::Arc;

use mms_config::Config;
use mms_execpolicy::Policy;
use mms_git::turn_diff::SharedTurnDiffTracker;
use mms_linux_sandbox::SandboxPolicy;

/// Context provided to tool handlers during execution.
///
/// Contains configuration, working directory, policies, and optional
/// turn diff tracker for recording file changes.
#[derive(Clone)]
pub struct ToolContext {
    config: Arc<Config>,
    cwd: PathBuf,
    timeout_ms: u64,
    sandbox_policy: SandboxPolicy,
    exec_policy: Option<Arc<Policy>>,
    sandbox_enabled: bool,
    /// Optional turn diff tracker for recording file changes during a turn.
    diff_tracker: Option<SharedTurnDiffTracker>,
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
            diff_tracker: None,
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

    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
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

    /// Set the turn diff tracker for recording file changes.
    pub fn with_diff_tracker(mut self, tracker: SharedTurnDiffTracker) -> Self {
        self.diff_tracker = Some(tracker);
        self
    }

    /// Get the turn diff tracker if set.
    pub fn diff_tracker(&self) -> Option<&SharedTurnDiffTracker> {
        self.diff_tracker.as_ref()
    }

    /// Record a file creation with the diff tracker.
    ///
    /// This is a convenience method that handles locking and checking
    /// if a tracker is active.
    pub async fn record_file_creation(&self, path: &PathBuf, tool_call_id: Option<&str>) {
        if let Some(tracker) = &self.diff_tracker {
            let mut guard = tracker.write().await;
            if let Some(ref mut tracker) = *guard {
                if let Some(id) = tool_call_id {
                    tracker.record_creation_with_tool(path, id);
                } else {
                    tracker.record_creation(path);
                }
            }
        }
    }

    /// Record a file modification with the diff tracker.
    ///
    /// The original content is used for undo functionality.
    pub async fn record_file_modification(
        &self,
        path: &PathBuf,
        original_content: Option<String>,
        tool_call_id: Option<&str>,
    ) {
        if let Some(tracker) = &self.diff_tracker {
            let mut guard = tracker.write().await;
            if let Some(ref mut tracker) = *guard {
                if let Some(id) = tool_call_id {
                    tracker.record_modification_with_tool(path, original_content, id);
                } else {
                    tracker.record_modification(path, original_content);
                }
            }
        }
    }

    /// Record a file deletion with the diff tracker.
    pub async fn record_file_deletion(
        &self,
        path: &PathBuf,
        original_content: Option<String>,
        tool_call_id: Option<&str>,
    ) {
        if let Some(tracker) = &self.diff_tracker {
            let mut guard = tracker.write().await;
            if let Some(ref mut tracker) = *guard {
                if let Some(id) = tool_call_id {
                    tracker.record_deletion_with_tool(path, original_content, id);
                } else {
                    tracker.record_deletion(path, original_content);
                }
            }
        }
    }
}
