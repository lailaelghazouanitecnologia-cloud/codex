use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Landlock ruleset creation failed")]
    LandlockRulesetCreate,

    #[error("Landlock rule addition failed: {0}")]
    LandlockRuleAdd(String),

    #[error("Landlock restrict_self failed")]
    LandlockRestrict,

    #[error("Landlock not enforced by kernel")]
    LandlockNotEnforced,

    #[error("Seccomp filter creation failed: {0}")]
    SeccompFilterCreate(String),

    #[error("Seccomp filter application failed: {0}")]
    SeccompApply(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Platform not supported")]
    PlatformNotSupported,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type SandboxResult<T> = Result<T, SandboxError>;
