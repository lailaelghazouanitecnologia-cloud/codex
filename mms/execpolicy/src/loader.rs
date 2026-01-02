use std::path::Path;

use serde::Deserialize;

use crate::decision::Decision;
use crate::error::{PolicyError, PolicyResult};
use crate::pattern::{CommandPattern, PatternToken};
use crate::policy::Policy;
use crate::rule::{ExactRule, GlobRule, PrefixRule};

/// TOML policy file format
#[derive(Debug, Deserialize)]
pub struct PolicyFile {
    #[serde(default)]
    pub policy: PolicySettings,
    #[serde(default)]
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PolicySettings {
    #[serde(default)]
    pub default: Option<Decision>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleEntry {
    Prefix(PrefixRuleEntry),
    Exact(ExactRuleEntry),
    Glob(GlobRuleEntry),
}

#[derive(Debug, Deserialize)]
pub struct PrefixRuleEntry {
    pub command: Vec<String>,
    pub decision: Decision,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExactRuleEntry {
    pub command: Vec<String>,
    pub decision: Decision,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GlobRuleEntry {
    pub program: String,
    pub decision: Decision,
    #[serde(default)]
    pub args: Option<Vec<ArgPattern>>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ArgPattern {
    Single(String),
    Alternatives(Vec<String>),
}

impl PolicyFile {
    /// Load a policy file from a path
    pub fn load(path: impl AsRef<Path>) -> PolicyResult<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| PolicyError::IoError(e.to_string()))?;
        Self::parse(&content)
    }

    /// Parse a policy file from string content
    pub fn parse(content: &str) -> PolicyResult<Self> {
        toml::from_str(content)
            .map_err(|e| PolicyError::ParseError(e.to_string()))
    }

    /// Convert to a Policy object
    pub fn into_policy(self) -> PolicyResult<Policy> {
        let default = self.policy.default.unwrap_or(Decision::Prompt);
        let mut policy = Policy::with_default(default);

        for entry in self.rules {
            match entry {
                RuleEntry::Prefix(entry) => {
                    if entry.command.is_empty() {
                        return Err(PolicyError::InvalidPattern("prefix command cannot be empty".into()));
                    }
                    let program = entry.command[0].clone();
                    let args: Vec<PatternToken> = entry.command[1..]
                        .iter()
                        .map(|s| PatternToken::Single(s.clone()))
                        .collect();
                    let pattern = CommandPattern { program, args };
                    policy.add_rule(PrefixRule::new(pattern, entry.decision));
                }
                RuleEntry::Exact(entry) => {
                    let rule = match entry.decision {
                        Decision::Allow => ExactRule::allow(entry.command),
                        Decision::Prompt => ExactRule::prompt(entry.command),
                        Decision::Forbidden => ExactRule::forbid(entry.command),
                    };
                    policy.add_rule(rule);
                }
                RuleEntry::Glob(entry) => {
                    let rule = match entry.decision {
                        Decision::Allow => GlobRule::allow_all(&entry.program),
                        Decision::Prompt => GlobRule::prompt_all(&entry.program),
                        Decision::Forbidden => GlobRule::forbid_all(&entry.program),
                    };
                    policy.add_rule(rule);
                }
            }
        }

        Ok(policy)
    }
}

/// Load a policy from a file path
pub fn load_policy(path: impl AsRef<Path>) -> PolicyResult<Policy> {
    PolicyFile::load(path)?.into_policy()
}

/// Parse a policy from string content
pub fn parse_policy(content: &str) -> PolicyResult<Policy> {
    PolicyFile::parse(content)?.into_policy()
}
