use std::path::Path;

use crate::error::SandboxResult;
use crate::landlock::install_filesystem_rules;
use crate::policy::SandboxPolicy;
use crate::seccomp::install_network_filter;

pub fn apply_sandbox_policy(
    policy: &SandboxPolicy,
    cwd: &Path,
) -> SandboxResult<()> {
    if !policy.has_full_network_access() {
        install_network_filter(policy.network_access, policy.allow_ptrace)?;
    }

    if !policy.has_full_disk_write_access() {
        let writable_roots = policy.get_writable_roots_with_cwd(cwd);
        install_filesystem_rules(writable_roots, !policy.allow_new_privs)?;
    }

    Ok(())
}
