use std::collections::BTreeMap;

use seccompiler::{
    apply_filter, BackendError, BpfProgram, SeccompAction, SeccompCmpArgLen,
    SeccompCmpOp, SeccompCondition, SeccompFilter, SeccompRule, TargetArch,
};

use crate::error::{SandboxError, SandboxResult};
use crate::policy::NetworkAccess;

pub fn install_network_filter(network_access: NetworkAccess, allow_ptrace: bool) -> SandboxResult<()> {
    match network_access {
        NetworkAccess::Full => return Ok(()),
        NetworkAccess::UnixOnly => install_unix_only_filter(allow_ptrace)?,
        NetworkAccess::None => install_no_network_filter(allow_ptrace)?,
    }

    Ok(())
}

fn install_unix_only_filter(allow_ptrace: bool) -> SandboxResult<()> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    let deny_syscall = |rules: &mut BTreeMap<i64, Vec<SeccompRule>>, nr: i64| {
        rules.insert(nr, vec![]);
    };

    deny_syscall(&mut rules, libc::SYS_connect);
    deny_syscall(&mut rules, libc::SYS_accept);
    deny_syscall(&mut rules, libc::SYS_accept4);
    deny_syscall(&mut rules, libc::SYS_bind);
    deny_syscall(&mut rules, libc::SYS_listen);
    deny_syscall(&mut rules, libc::SYS_getpeername);
    deny_syscall(&mut rules, libc::SYS_getsockname);
    deny_syscall(&mut rules, libc::SYS_shutdown);
    deny_syscall(&mut rules, libc::SYS_sendto);
    deny_syscall(&mut rules, libc::SYS_sendmmsg);
    deny_syscall(&mut rules, libc::SYS_recvmmsg);
    deny_syscall(&mut rules, libc::SYS_getsockopt);
    deny_syscall(&mut rules, libc::SYS_setsockopt);

    if !allow_ptrace {
        deny_syscall(&mut rules, libc::SYS_ptrace);
    }

    let unix_only_rule = create_unix_only_rule()?;

    rules.insert(libc::SYS_socket, vec![unix_only_rule.clone()]);
    rules.insert(libc::SYS_socketpair, vec![unix_only_rule]);

    apply_seccomp_filter(rules)
}

fn install_no_network_filter(allow_ptrace: bool) -> SandboxResult<()> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    let deny_syscall = |rules: &mut BTreeMap<i64, Vec<SeccompRule>>, nr: i64| {
        rules.insert(nr, vec![]);
    };

    deny_syscall(&mut rules, libc::SYS_socket);
    deny_syscall(&mut rules, libc::SYS_socketpair);
    deny_syscall(&mut rules, libc::SYS_connect);
    deny_syscall(&mut rules, libc::SYS_accept);
    deny_syscall(&mut rules, libc::SYS_accept4);
    deny_syscall(&mut rules, libc::SYS_bind);
    deny_syscall(&mut rules, libc::SYS_listen);
    deny_syscall(&mut rules, libc::SYS_getpeername);
    deny_syscall(&mut rules, libc::SYS_getsockname);
    deny_syscall(&mut rules, libc::SYS_shutdown);
    deny_syscall(&mut rules, libc::SYS_sendto);
    deny_syscall(&mut rules, libc::SYS_sendmsg);
    deny_syscall(&mut rules, libc::SYS_sendmmsg);
    deny_syscall(&mut rules, libc::SYS_recvfrom);
    deny_syscall(&mut rules, libc::SYS_recvmsg);
    deny_syscall(&mut rules, libc::SYS_recvmmsg);
    deny_syscall(&mut rules, libc::SYS_getsockopt);
    deny_syscall(&mut rules, libc::SYS_setsockopt);

    if !allow_ptrace {
        deny_syscall(&mut rules, libc::SYS_ptrace);
    }

    apply_seccomp_filter(rules)
}

fn create_unix_only_rule() -> SandboxResult<SeccompRule> {
    let condition = SeccompCondition::new(
        0,
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Ne,
        libc::AF_UNIX as u64,
    )
    .map_err(|e| SandboxError::SeccompFilterCreate(e.to_string()))?;

    SeccompRule::new(vec![condition])
        .map_err(|e| SandboxError::SeccompFilterCreate(e.to_string()))
}

fn apply_seccomp_filter(rules: BTreeMap<i64, Vec<SeccompRule>>) -> SandboxResult<()> {
    let arch = get_target_arch();

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )
    .map_err(|e| SandboxError::SeccompFilterCreate(e.to_string()))?;

    let prog: BpfProgram = filter
        .try_into()
        .map_err(|e: BackendError| SandboxError::SeccompFilterCreate(e.to_string()))?;

    apply_filter(&prog)
        .map_err(|e| SandboxError::SeccompApply(e.to_string()))?;

    Ok(())
}

fn get_target_arch() -> TargetArch {
    #[cfg(target_arch = "x86_64")]
    {
        TargetArch::x86_64
    }

    #[cfg(target_arch = "aarch64")]
    {
        TargetArch::aarch64
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        compile_error!("Unsupported architecture for seccomp");
    }
}
