//! Execution policy manager for session-level policy management.

use std::path::Path;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::decision::Decision;
use crate::error::PolicyResult;
use crate::loader::{PolicySettings, load_policy};
use crate::policy::{Evaluation, Policy};

/// Manager for execution policies within a session.
///
/// This provides a higher-level interface for policy management,
/// including loading from config files and async checking.
pub struct ExecPolicyManager {
    /// The underlying policy.
    policy: Arc<RwLock<Policy>>,
    /// Settings used to load the policy.
    settings: Option<PolicySettings>,
}

impl ExecPolicyManager {
    /// Create a new empty policy manager.
    pub fn new() -> Self {
        Self {
            policy: Arc::new(RwLock::new(Policy::new())),
            settings: None,
        }
    }

    /// Create a policy manager with a given policy.
    pub fn with_policy(policy: Policy) -> Self {
        Self {
            policy: Arc::new(RwLock::new(policy)),
            settings: None,
        }
    }

    /// Load a policy from a file.
    pub async fn load(path: &Path) -> PolicyResult<Self> {
        let policy = load_policy(path)?;
        Ok(Self {
            policy: Arc::new(RwLock::new(policy)),
            settings: None,
        })
    }

    /// Load policy from default locations.
    pub async fn load_default() -> PolicyResult<Self> {
        // Try to load from standard locations
        let home = dirs::home_dir();

        if let Some(home) = home {
            // Try ~/.config/agent/execpolicy.toml
            let config_path = home.join(".config/agent/execpolicy.toml");
            if config_path.exists() {
                return Self::load(&config_path).await;
            }

            // Try ~/.agent/execpolicy.toml
            let agent_path = home.join(".agent/execpolicy.toml");
            if agent_path.exists() {
                return Self::load(&agent_path).await;
            }
        }

        // Return default policy
        Ok(Self::new())
    }

    /// Check a command against the policy.
    pub async fn check(&self, command: &str) -> Evaluation {
        let parts: Vec<String> = shell_words::split(command)
            .unwrap_or_else(|_| vec![command.to_string()]);

        let policy = self.policy.read().await;
        policy.check(&parts)
    }

    /// Check multiple commands against the policy.
    pub async fn check_many(&self, commands: &[&str]) -> Evaluation {
        let all_parts: Vec<Vec<String>> = commands
            .iter()
            .map(|cmd| {
                shell_words::split(cmd).unwrap_or_else(|_| vec![cmd.to_string()])
            })
            .collect();

        let refs: Vec<&[String]> = all_parts.iter().map(|v| v.as_slice()).collect();

        let policy = self.policy.read().await;
        policy.check_many(refs)
    }

    /// Check if a command is allowed.
    pub async fn is_allowed(&self, command: &str) -> bool {
        self.check(command).await.is_allowed()
    }

    /// Check if a command is forbidden.
    pub async fn is_forbidden(&self, command: &str) -> bool {
        self.check(command).await.is_forbidden()
    }

    /// Check if a command requires a prompt.
    pub async fn requires_prompt(&self, command: &str) -> bool {
        self.check(command).await.requires_prompt()
    }

    /// Get the decision for a command.
    pub async fn get_decision(&self, command: &str) -> Decision {
        self.check(command).await.decision
    }

    /// Add an allow rule for a command prefix.
    pub async fn allow_prefix(&self, prefix: &[String]) -> PolicyResult<()> {
        let mut policy = self.policy.write().await;
        policy.allow_prefix(prefix)?;
        Ok(())
    }

    /// Add a forbid rule for a command prefix.
    pub async fn forbid_prefix(&self, prefix: &[String]) -> PolicyResult<()> {
        let mut policy = self.policy.write().await;
        policy.forbid_prefix(prefix)?;
        Ok(())
    }

    /// Get the number of rules in the policy.
    pub async fn rule_count(&self) -> usize {
        self.policy.read().await.rule_count()
    }

    /// Check if the policy is empty.
    pub async fn is_empty(&self) -> bool {
        self.policy.read().await.is_empty()
    }

    /// Get the settings used to load the policy.
    pub fn settings(&self) -> Option<&PolicySettings> {
        self.settings.as_ref()
    }

    /// Replace the underlying policy.
    pub async fn set_policy(&self, policy: Policy) {
        *self.policy.write().await = policy;
    }

    /// Get a clone of the underlying policy.
    pub async fn get_policy(&self) -> Policy {
        self.policy.read().await.clone()
    }
}

impl Default for ExecPolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ExecPolicyManager {
    fn clone(&self) -> Self {
        Self {
            policy: Arc::clone(&self.policy),
            settings: self.settings.clone(),
        }
    }
}
