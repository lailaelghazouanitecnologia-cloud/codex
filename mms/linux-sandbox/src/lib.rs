mod policy;
mod error;

#[cfg(target_os = "linux")]
mod landlock;
#[cfg(target_os = "linux")]
mod seccomp;
#[cfg(target_os = "linux")]
mod apply;

pub use error::{SandboxError, SandboxResult};
pub use policy::{SandboxPolicy, SandboxConfig, WritableRoot, NetworkAccess, DiskAccess};

#[cfg(target_os = "linux")]
pub use apply::apply_sandbox_policy;

#[cfg(target_os = "linux")]
pub use landlock::is_landlock_supported;

#[cfg(not(target_os = "linux"))]
pub fn apply_sandbox_policy(
    _policy: &SandboxPolicy,
    _cwd: &std::path::Path,
) -> SandboxResult<()> {
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn is_landlock_supported() -> bool {
    false
}

pub fn is_sandbox_supported() -> bool {
    cfg!(target_os = "linux")
}
