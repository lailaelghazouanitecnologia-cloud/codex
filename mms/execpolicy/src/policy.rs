use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::decision::Decision;
use crate::error::{PolicyError, PolicyResult};
use crate::pattern::{CommandPattern, PatternToken};
use crate::rule::{PrefixRule, Rule, RuleMatch, RuleRef};

pub struct Policy {
    rules_by_program: HashMap<String, Vec<RuleRef>>,
    default_decision: Decision,
}

impl Policy {
    pub fn new() -> Self {
        Self {
            rules_by_program: HashMap::new(),
            default_decision: Decision::Prompt,
        }
    }

    pub fn with_default(default: Decision) -> Self {
        Self {
            rules_by_program: HashMap::new(),
            default_decision: default,
        }
    }

    pub fn add_rule(&mut self, rule: impl Rule + 'static) -> &mut Self {
        let program = rule.program().to_string();
        self.rules_by_program
            .entry(program)
            .or_default()
            .push(Arc::new(rule));
        self
    }

    pub fn add_prefix_rule(&mut self, prefix: &[String], decision: Decision) -> PolicyResult<&mut Self> {
        if prefix.is_empty() {
            return Err(PolicyError::InvalidPattern("prefix cannot be empty".into()));
        }

        let program = prefix[0].clone();
        let args: Vec<PatternToken> = prefix[1..]
            .iter()
            .map(|s| PatternToken::Single(s.clone()))
            .collect();

        let pattern = CommandPattern {
            program: program.clone(),
            args,
        };

        let rule = PrefixRule::new(pattern, decision);
        self.add_rule(rule);
        Ok(self)
    }

    pub fn allow_prefix(&mut self, prefix: &[String]) -> PolicyResult<&mut Self> {
        self.add_prefix_rule(prefix, Decision::Allow)
    }

    pub fn forbid_prefix(&mut self, prefix: &[String]) -> PolicyResult<&mut Self> {
        self.add_prefix_rule(prefix, Decision::Forbidden)
    }

    pub fn check(&self, command: &[String]) -> Evaluation {
        let matches = self.get_matches(command);
        Evaluation::from_matches(matches, self.default_decision)
    }

    pub fn check_many<'a>(&self, commands: impl IntoIterator<Item = &'a [String]>) -> Evaluation {
        let matches: Vec<RuleMatch> = commands
            .into_iter()
            .flat_map(|cmd| self.get_matches(cmd))
            .collect();

        Evaluation::from_matches(matches, self.default_decision)
    }

    fn get_matches(&self, command: &[String]) -> Vec<RuleMatch> {
        let program = match command.first() {
            Some(p) => p,
            None => return Vec::new(),
        };

        self.rules_by_program
            .get(program)
            .map(|rules| {
                rules
                    .iter()
                    .filter_map(|rule| rule.matches(command))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.rules_by_program.is_empty()
    }

    pub fn rule_count(&self) -> usize {
        self.rules_by_program.values().map(|v| v.len()).sum()
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    pub decision: Decision,
    pub matched_rules: Vec<RuleMatch>,
}

impl Evaluation {
    pub fn from_matches(matches: Vec<RuleMatch>, default: Decision) -> Self {
        let decision = matches
            .iter()
            .map(RuleMatch::decision)
            .max()
            .unwrap_or(default);

        Self {
            decision,
            matched_rules: matches,
        }
    }

    pub fn is_allowed(&self) -> bool {
        self.decision.is_allowed()
    }

    pub fn requires_prompt(&self) -> bool {
        self.decision.requires_prompt()
    }

    pub fn is_forbidden(&self) -> bool {
        self.decision.is_forbidden()
    }

    pub fn has_matches(&self) -> bool {
        !self.matched_rules.is_empty()
    }
}

pub fn default_heuristics(command: &[String]) -> Decision {
    if command.is_empty() {
        return Decision::Forbidden;
    }

    let program = &command[0];

    let dangerous_programs = [
        "rm", "rmdir", "mv", "chmod", "chown",
        "sudo", "su", "pkill", "kill", "killall",
        "dd", "mkfs", "fdisk", "parted",
        "iptables", "ip6tables", "nft",
        "systemctl", "service",
        "reboot", "shutdown", "halt", "poweroff",
        "curl", "wget", "nc", "ncat", "netcat",
        "ssh", "scp", "rsync",
    ];

    let safe_programs = [
        "ls", "cat", "head", "tail", "less", "more",
        "grep", "rg", "ag", "ack",
        "find", "fd", "locate",
        "wc", "sort", "uniq", "cut", "tr",
        "echo", "printf", "date", "cal",
        "pwd", "whoami", "hostname",
        "file", "stat", "du", "df",
        "diff", "cmp", "comm",
        "jq", "yq", "xq",
        "git", "hg", "svn",
        "node", "npm", "npx", "yarn", "pnpm",
        "python", "python3", "pip", "pip3",
        "cargo", "rustc", "rustfmt", "clippy",
        "go", "gofmt",
        "make", "cmake", "ninja",
    ];

    if dangerous_programs.contains(&program.as_str()) {
        return Decision::Prompt;
    }

    if safe_programs.contains(&program.as_str()) {
        return Decision::Allow;
    }

    Decision::Prompt
}
