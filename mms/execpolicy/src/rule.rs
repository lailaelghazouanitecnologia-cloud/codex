use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::decision::Decision;
use crate::pattern::CommandPattern;

pub trait Rule: Send + Sync + std::fmt::Debug {
    fn program(&self) -> &str;
    fn matches(&self, command: &[String]) -> Option<RuleMatch>;
    fn decision(&self) -> Decision;
}

pub type RuleRef = Arc<dyn Rule>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleMatch {
    Prefix {
        matched_prefix: Vec<String>,
        decision: Decision,
    },
    Exact {
        command: Vec<String>,
        decision: Decision,
    },
    Heuristic {
        command: Vec<String>,
        decision: Decision,
        reason: String,
    },
}

impl RuleMatch {
    pub fn prefix(matched: Vec<String>, decision: Decision) -> Self {
        Self::Prefix {
            matched_prefix: matched,
            decision,
        }
    }

    pub fn exact(command: Vec<String>, decision: Decision) -> Self {
        Self::Exact { command, decision }
    }

    pub fn heuristic(command: Vec<String>, decision: Decision, reason: impl Into<String>) -> Self {
        Self::Heuristic {
            command,
            decision,
            reason: reason.into(),
        }
    }

    pub fn decision(&self) -> Decision {
        match self {
            Self::Prefix { decision, .. } => *decision,
            Self::Exact { decision, .. } => *decision,
            Self::Heuristic { decision, .. } => *decision,
        }
    }

    pub fn command(&self) -> &[String] {
        match self {
            Self::Prefix { matched_prefix, .. } => matched_prefix,
            Self::Exact { command, .. } => command,
            Self::Heuristic { command, .. } => command,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrefixRule {
    pub pattern: CommandPattern,
    pub decision: Decision,
}

impl PrefixRule {
    pub fn new(pattern: CommandPattern, decision: Decision) -> Self {
        Self { pattern, decision }
    }

    pub fn allow(pattern: CommandPattern) -> Self {
        Self::new(pattern, Decision::Allow)
    }

    pub fn prompt(pattern: CommandPattern) -> Self {
        Self::new(pattern, Decision::Prompt)
    }

    pub fn forbid(pattern: CommandPattern) -> Self {
        Self::new(pattern, Decision::Forbidden)
    }
}

impl Rule for PrefixRule {
    fn program(&self) -> &str {
        &self.pattern.program
    }

    fn matches(&self, command: &[String]) -> Option<RuleMatch> {
        self.pattern.matches(command).map(|matched| {
            RuleMatch::prefix(matched, self.decision)
        })
    }

    fn decision(&self) -> Decision {
        self.decision
    }
}

#[derive(Debug, Clone)]
pub struct ExactRule {
    pub command: Vec<String>,
    pub decision: Decision,
}

impl ExactRule {
    pub fn new(command: Vec<String>, decision: Decision) -> Self {
        Self { command, decision }
    }

    pub fn allow(command: Vec<String>) -> Self {
        Self::new(command, Decision::Allow)
    }

    pub fn prompt(command: Vec<String>) -> Self {
        Self::new(command, Decision::Prompt)
    }

    pub fn forbid(command: Vec<String>) -> Self {
        Self::new(command, Decision::Forbidden)
    }
}

impl Rule for ExactRule {
    fn program(&self) -> &str {
        self.command.first().map(String::as_str).unwrap_or("")
    }

    fn matches(&self, command: &[String]) -> Option<RuleMatch> {
        if command == self.command {
            Some(RuleMatch::exact(command.to_vec(), self.decision))
        } else {
            None
        }
    }

    fn decision(&self) -> Decision {
        self.decision
    }
}

#[derive(Debug, Clone)]
pub struct GlobRule {
    pub program: String,
    pub decision: Decision,
}

impl GlobRule {
    pub fn new(program: impl Into<String>, decision: Decision) -> Self {
        Self {
            program: program.into(),
            decision,
        }
    }

    pub fn allow_all(program: impl Into<String>) -> Self {
        Self::new(program, Decision::Allow)
    }

    pub fn prompt_all(program: impl Into<String>) -> Self {
        Self::new(program, Decision::Prompt)
    }

    pub fn forbid_all(program: impl Into<String>) -> Self {
        Self::new(program, Decision::Forbidden)
    }
}

impl Rule for GlobRule {
    fn program(&self) -> &str {
        &self.program
    }

    fn matches(&self, command: &[String]) -> Option<RuleMatch> {
        if command.first().map(String::as_str) == Some(&self.program) {
            Some(RuleMatch::prefix(vec![self.program.clone()], self.decision))
        } else {
            None
        }
    }

    fn decision(&self) -> Decision {
        self.decision
    }
}
