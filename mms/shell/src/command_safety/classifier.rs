//! Command classification for approval flow.
//!
//! This module combines safe and dangerous command detection
//! with approval policies to determine how commands should be handled.

use super::dangerous_commands::{is_dangerous_to_exec, DangerKind, DangerousPattern};
use super::safe_commands::{is_safe_to_exec, SafeCommandResult};

/// Approval policy for command execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalPolicy {
    /// Never approve any command automatically.
    /// All commands require explicit approval.
    Never,

    /// Always approve all commands automatically.
    /// No approval required (dangerous - use with caution).
    Always,

    /// Approve safe commands, prompt for others.
    /// This is the default behavior.
    #[default]
    OnDanger,

    /// Only prompt for explicitly dangerous commands.
    /// Safe and unknown commands are auto-approved.
    OnlyDangerous,

    /// Require approval unless command is known safe.
    /// More conservative than OnDanger.
    UnlessTrusted,
}

impl ApprovalPolicy {
    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "never" => Some(Self::Never),
            "always" => Some(Self::Always),
            "on_danger" | "ondanger" => Some(Self::OnDanger),
            "only_dangerous" | "onlydangerous" => Some(Self::OnlyDangerous),
            "unless_trusted" | "unlesstrusted" => Some(Self::UnlessTrusted),
            _ => None,
        }
    }

    /// Convert to string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Always => "always",
            Self::OnDanger => "on_danger",
            Self::OnlyDangerous => "only_dangerous",
            Self::UnlessTrusted => "unless_trusted",
        }
    }
}

/// Result of command classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandClassification {
    /// Command is safe and can be executed without approval.
    Safe,

    /// Command needs approval before execution.
    NeedsApproval {
        /// Reason why approval is needed.
        reason: String,
        /// Risk assessment.
        risk: CommandRisk,
    },

    /// Command is forbidden and should not be executed.
    Forbidden {
        /// Reason why the command is forbidden.
        reason: String,
    },
}

impl CommandClassification {
    /// Check if the command is safe.
    pub fn is_safe(&self) -> bool {
        matches!(self, Self::Safe)
    }

    /// Check if the command needs approval.
    pub fn needs_approval(&self) -> bool {
        matches!(self, Self::NeedsApproval { .. })
    }

    /// Check if the command is forbidden.
    pub fn is_forbidden(&self) -> bool {
        matches!(self, Self::Forbidden { .. })
    }
}

/// Risk assessment for a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRisk {
    /// Overall risk level (1-5).
    pub level: u8,
    /// Risk factors identified.
    pub factors: Vec<RiskFactor>,
    /// Command category.
    pub category: Option<DangerKind>,
}

impl CommandRisk {
    /// Create a new risk assessment.
    pub fn new(level: u8) -> Self {
        Self {
            level: level.min(5).max(1),
            factors: Vec::new(),
            category: None,
        }
    }

    /// Add a risk factor.
    pub fn with_factor(mut self, factor: RiskFactor) -> Self {
        self.factors.push(factor);
        self
    }

    /// Set the category.
    pub fn with_category(mut self, category: DangerKind) -> Self {
        self.category = Some(category);
        self
    }

    /// Get risk level as string.
    pub fn level_str(&self) -> &'static str {
        match self.level {
            1 => "low",
            2 => "medium-low",
            3 => "medium",
            4 => "high",
            5 => "critical",
            _ => "unknown",
        }
    }
}

impl Default for CommandRisk {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Individual risk factors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskFactor {
    /// Command modifies files.
    ModifiesFiles,
    /// Command deletes files.
    DeletesFiles,
    /// Command requires elevated privileges.
    ElevatedPrivileges,
    /// Command accesses network.
    NetworkAccess,
    /// Command executes external code.
    ExecutesCode,
    /// Command affects system configuration.
    SystemConfiguration,
    /// Command affects other processes.
    ProcessControl,
    /// Command is not recognized.
    UnknownCommand,
    /// Command uses dangerous patterns.
    DangerousPattern(String),
}

impl RiskFactor {
    /// Get human-readable description.
    pub fn description(&self) -> String {
        match self {
            Self::ModifiesFiles => "modifies files".to_string(),
            Self::DeletesFiles => "deletes files".to_string(),
            Self::ElevatedPrivileges => "requires elevated privileges".to_string(),
            Self::NetworkAccess => "accesses network".to_string(),
            Self::ExecutesCode => "executes external code".to_string(),
            Self::SystemConfiguration => "affects system configuration".to_string(),
            Self::ProcessControl => "controls processes".to_string(),
            Self::UnknownCommand => "unrecognized command".to_string(),
            Self::DangerousPattern(p) => format!("dangerous pattern: {}", p),
        }
    }
}

/// Execution context for command classification.
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    /// Current working directory.
    pub cwd: Option<String>,
    /// Whether running in sandbox.
    pub sandboxed: bool,
    /// User's home directory.
    pub home_dir: Option<String>,
    /// Additional allowed commands.
    pub allowed_commands: Vec<String>,
    /// Additional forbidden commands.
    pub forbidden_commands: Vec<String>,
}

impl ExecutionContext {
    /// Create new context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the working directory.
    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set sandboxed mode.
    pub fn with_sandbox(mut self, sandboxed: bool) -> Self {
        self.sandboxed = sandboxed;
        self
    }
}

/// Classify a command for approval.
///
/// This is the main entry point for command classification.
///
/// # Arguments
///
/// * `command` - The command as a slice of strings
/// * `policy` - The approval policy to apply
///
/// # Returns
///
/// A [`CommandClassification`] indicating how the command should be handled.
///
/// # Example
///
/// ```rust
/// use mms_shell::command_safety::{classify_command, ApprovalPolicy, CommandClassification};
///
/// let result = classify_command(&["ls", "-la"], ApprovalPolicy::OnDanger);
/// assert!(result.is_safe());
///
/// let result = classify_command(&["rm", "-rf", "/"], ApprovalPolicy::OnDanger);
/// assert!(result.is_forbidden());
/// ```
pub fn classify_command(command: &[&str], policy: ApprovalPolicy) -> CommandClassification {
    classify_command_with_context(command, policy, &ExecutionContext::default())
}

/// Classify a command with execution context.
///
/// This version allows providing additional context that affects classification.
///
/// # Arguments
///
/// * `command` - The command as a slice of strings
/// * `policy` - The approval policy to apply
/// * `context` - Execution context
///
/// # Returns
///
/// A [`CommandClassification`] indicating how the command should be handled.
pub fn classify_command_with_context(
    command: &[&str],
    policy: ApprovalPolicy,
    context: &ExecutionContext,
) -> CommandClassification {
    if command.is_empty() {
        return CommandClassification::Forbidden {
            reason: "empty command".to_string(),
        };
    }

    // Check custom forbidden list
    let base_cmd = std::path::Path::new(command[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command[0]);

    if context.forbidden_commands.iter().any(|c| c == base_cmd) {
        return CommandClassification::Forbidden {
            reason: format!("{} is in forbidden list", base_cmd),
        };
    }

    // Check for explicitly dangerous commands first
    let dangerous = is_dangerous_to_exec(command);
    let safe = is_safe_to_exec(command);

    // Check for critical severity (forbidden regardless of policy)
    if let Some(ref pattern) = dangerous {
        if pattern.severity >= 5 {
            return CommandClassification::Forbidden {
                reason: pattern.reason.clone(),
            };
        }
    }

    // Apply policy
    match policy {
        ApprovalPolicy::Never => {
            // All commands require approval
            let risk = build_risk(&dangerous, &safe);
            CommandClassification::NeedsApproval {
                reason: "approval policy requires all commands to be approved".to_string(),
                risk,
            }
        }

        ApprovalPolicy::Always => {
            // Check custom allowed list
            if context.allowed_commands.iter().any(|c| c == base_cmd) {
                return CommandClassification::Safe;
            }

            // Even with Always, forbid critical commands
            if let Some(ref pattern) = dangerous {
                if pattern.severity >= 4 {
                    let risk = build_risk(&dangerous, &safe);
                    return CommandClassification::NeedsApproval {
                        reason: pattern.reason.clone(),
                        risk,
                    };
                }
            }

            CommandClassification::Safe
        }

        ApprovalPolicy::OnDanger => {
            // Check if safe first
            if safe.is_safe() && dangerous.is_none() {
                return CommandClassification::Safe;
            }

            // Check custom allowed list
            if context.allowed_commands.iter().any(|c| c == base_cmd) {
                return CommandClassification::Safe;
            }

            // If dangerous or not safe, require approval
            let risk = build_risk(&dangerous, &safe);
            let reason = match (&dangerous, &safe) {
                (Some(d), _) => d.reason.clone(),
                (_, SafeCommandResult::UnsafeArguments { reason }) => reason.clone(),
                (_, SafeCommandResult::NotSafe) => format!("{} is not a known safe command", base_cmd),
                _ => "command requires approval".to_string(),
            };

            CommandClassification::NeedsApproval { reason, risk }
        }

        ApprovalPolicy::OnlyDangerous => {
            // Only require approval for explicitly dangerous commands
            if let Some(pattern) = dangerous {
                let risk = build_risk(&Some(pattern.clone()), &safe);
                CommandClassification::NeedsApproval {
                    reason: pattern.reason,
                    risk,
                }
            } else {
                CommandClassification::Safe
            }
        }

        ApprovalPolicy::UnlessTrusted => {
            // Only safe if explicitly known safe
            if safe.is_safe() {
                CommandClassification::Safe
            } else {
                let risk = build_risk(&dangerous, &safe);
                let reason = match (&dangerous, &safe) {
                    (Some(d), _) => d.reason.clone(),
                    (_, SafeCommandResult::UnsafeArguments { reason }) => reason.clone(),
                    _ => format!("{} is not in trusted command list", base_cmd),
                };
                CommandClassification::NeedsApproval { reason, risk }
            }
        }
    }
}

/// Build risk assessment from analysis results.
fn build_risk(
    dangerous: &Option<DangerousPattern>,
    safe: &SafeCommandResult,
) -> CommandRisk {
    let mut risk = CommandRisk::default();

    // Set level based on dangerous pattern severity
    if let Some(pattern) = dangerous {
        risk.level = pattern.severity;
        risk.category = Some(pattern.kind.clone());

        // Add risk factors based on category
        match pattern.kind {
            DangerKind::FileDestruction => {
                risk.factors.push(RiskFactor::DeletesFiles);
            }
            DangerKind::SystemModification => {
                risk.factors.push(RiskFactor::SystemConfiguration);
            }
            DangerKind::PrivilegeEscalation => {
                risk.factors.push(RiskFactor::ElevatedPrivileges);
            }
            DangerKind::NetworkExfiltration => {
                risk.factors.push(RiskFactor::NetworkAccess);
            }
            DangerKind::RemoteCodeExecution => {
                risk.factors.push(RiskFactor::ExecutesCode);
            }
            DangerKind::DiskOperation => {
                risk.factors.push(RiskFactor::ModifiesFiles);
            }
            DangerKind::ProcessControl => {
                risk.factors.push(RiskFactor::ProcessControl);
            }
            DangerKind::GitDestructive => {
                risk.factors.push(RiskFactor::ModifiesFiles);
            }
            DangerKind::PackageManager => {
                risk.factors.push(RiskFactor::SystemConfiguration);
            }
            DangerKind::ContainerOperation => {
                risk.factors.push(RiskFactor::ProcessControl);
            }
            DangerKind::PotentiallyHarmful => {
                risk.factors.push(RiskFactor::DangerousPattern(pattern.reason.clone()));
            }
        }
    }

    // If not safe and no specific dangerous pattern, mark as unknown
    if !safe.is_safe() && dangerous.is_none() {
        risk.level = risk.level.max(2);
        risk.factors.push(RiskFactor::UnknownCommand);
    }

    // Adjust level based on unsafe arguments
    if let SafeCommandResult::UnsafeArguments { reason } = safe {
        risk.level = risk.level.max(2);
        risk.factors.push(RiskFactor::DangerousPattern(reason.clone()));
    }

    risk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command_classification() {
        let result = classify_command(&["ls", "-la"], ApprovalPolicy::OnDanger);
        assert!(result.is_safe());
    }

    #[test]
    fn test_dangerous_command_needs_approval() {
        let result = classify_command(&["rm", "-f", "file.txt"], ApprovalPolicy::OnDanger);
        assert!(result.needs_approval());
    }

    #[test]
    fn test_critical_command_forbidden() {
        let result = classify_command(&["rm", "-rf", "/"], ApprovalPolicy::OnDanger);
        assert!(result.is_forbidden());
    }

    #[test]
    fn test_never_policy() {
        let result = classify_command(&["ls"], ApprovalPolicy::Never);
        assert!(result.needs_approval());
    }

    #[test]
    fn test_always_policy_with_dangerous() {
        let result = classify_command(&["sudo", "rm", "-rf", "/tmp"], ApprovalPolicy::Always);
        // High severity should still require approval
        assert!(result.needs_approval());
    }

    #[test]
    fn test_only_dangerous_policy() {
        // Safe command
        let result = classify_command(&["cat", "file.txt"], ApprovalPolicy::OnlyDangerous);
        assert!(result.is_safe());

        // Unknown but not dangerous
        let result = classify_command(&["myapp", "--version"], ApprovalPolicy::OnlyDangerous);
        assert!(result.is_safe());

        // Explicitly dangerous
        let result = classify_command(&["sudo", "bash"], ApprovalPolicy::OnlyDangerous);
        assert!(result.needs_approval());
    }

    #[test]
    fn test_unless_trusted_policy() {
        // Known safe
        let result = classify_command(&["pwd"], ApprovalPolicy::UnlessTrusted);
        assert!(result.is_safe());

        // Unknown command
        let result = classify_command(&["myapp"], ApprovalPolicy::UnlessTrusted);
        assert!(result.needs_approval());
    }

    #[test]
    fn test_context_forbidden() {
        let context = ExecutionContext {
            forbidden_commands: vec!["dangerous".to_string()],
            ..Default::default()
        };
        let result = classify_command_with_context(
            &["dangerous", "--help"],
            ApprovalPolicy::Always,
            &context,
        );
        assert!(result.is_forbidden());
    }

    #[test]
    fn test_context_allowed() {
        let context = ExecutionContext {
            allowed_commands: vec!["myapp".to_string()],
            ..Default::default()
        };
        let result = classify_command_with_context(
            &["myapp", "--action"],
            ApprovalPolicy::OnDanger,
            &context,
        );
        assert!(result.is_safe());
    }
}
