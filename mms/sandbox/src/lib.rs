mod executor;
mod policy;
mod container;

pub use executor::{SandboxExecutor, ExecutionResult, CommandCheck};
pub use policy::{SandboxPolicy, Permission};
pub use container::{SandboxContainer, ContainerState, ContainerError};

pub mod linux {
    pub use mms_linux_sandbox::{
        SandboxPolicy as LinuxSandboxPolicy,
        SandboxConfig,
        WritableRoot,
        NetworkAccess,
        DiskAccess,
        SandboxError,
        SandboxResult,
        apply_sandbox_policy,
        is_sandbox_supported,
        is_landlock_supported,
    };
}

pub mod execpolicy {
    pub use mms_execpolicy::{
        Decision,
        Policy,
        Evaluation,
        Rule,
        RuleRef,
        PrefixRule,
        ExactRule,
        GlobRule,
        RuleMatch,
        PatternToken,
        CommandPattern,
        PolicyError,
        PolicyResult,
        default_heuristics,
    };
}
