use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternToken {
    Single(String),
    Alternatives(Vec<String>),
}

impl PatternToken {
    pub fn single(s: impl Into<String>) -> Self {
        Self::Single(s.into())
    }

    pub fn alternatives(alts: Vec<String>) -> Self {
        Self::Alternatives(alts)
    }

    pub fn matches(&self, token: &str) -> bool {
        match self {
            Self::Single(expected) => expected == token,
            Self::Alternatives(alts) => alts.iter().any(|alt| alt == token),
        }
    }

    pub fn values(&self) -> Vec<&str> {
        match self {
            Self::Single(s) => vec![s.as_str()],
            Self::Alternatives(alts) => alts.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPattern {
    pub program: String,
    pub args: Vec<PatternToken>,
}

impl CommandPattern {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn with_arg(mut self, arg: PatternToken) -> Self {
        self.args.push(arg);
        self
    }

    pub fn with_single_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(PatternToken::Single(arg.into()));
        self
    }

    pub fn matches(&self, command: &[String]) -> Option<Vec<String>> {
        if command.is_empty() {
            return None;
        }

        if command[0] != self.program {
            return None;
        }

        let pattern_len = self.args.len() + 1;

        if command.len() < pattern_len {
            return None;
        }

        for (pattern_token, cmd_token) in self.args.iter().zip(&command[1..pattern_len]) {
            if !pattern_token.matches(cmd_token) {
                return None;
            }
        }

        Some(command[..pattern_len].to_vec())
    }

    pub fn len(&self) -> usize {
        self.args.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

impl std::fmt::Display for CommandPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.program)?;

        for arg in &self.args {
            match arg {
                PatternToken::Single(s) => write!(f, " {}", s)?,
                PatternToken::Alternatives(alts) => write!(f, " ({})", alts.join("|"))?,
            }
        }

        Ok(())
    }
}
