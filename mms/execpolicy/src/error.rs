use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Invalid decision: {0}")]
    InvalidDecision(String),

    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("Rule not found: {0}")]
    RuleNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Example did not match: {0}")]
    ExampleNotMatched(String),

    #[error("IO error: {0}")]
    IoError(String),
}

pub type PolicyResult<T> = Result<T, PolicyError>;
