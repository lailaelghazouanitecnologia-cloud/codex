use serde::{Deserialize, Serialize};

use crate::error::{PolicyError, PolicyResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Prompt,
    Forbidden,
}

impl Decision {
    pub fn parse(s: &str) -> PolicyResult<Self> {
        match s.to_lowercase().as_str() {
            "allow" => Ok(Self::Allow),
            "prompt" => Ok(Self::Prompt),
            "forbidden" | "deny" | "block" => Ok(Self::Forbidden),
            _ => Err(PolicyError::InvalidDecision(s.to_string())),
        }
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }

    pub fn requires_prompt(&self) -> bool {
        matches!(self, Self::Prompt)
    }

    pub fn is_forbidden(&self) -> bool {
        matches!(self, Self::Forbidden)
    }

    pub fn merge(self, other: Self) -> Self {
        std::cmp::max(self, other)
    }
}

impl Default for Decision {
    fn default() -> Self {
        Self::Prompt
    }
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Prompt => write!(f, "prompt"),
            Self::Forbidden => write!(f, "forbidden"),
        }
    }
}
