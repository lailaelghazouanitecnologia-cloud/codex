use mms_linux_sandbox::{
    SandboxPolicy, SandboxConfig, WritableRoot, NetworkAccess, DiskAccess,
    apply_sandbox_policy, is_sandbox_supported,
};
use std::path::PathBuf;

#[test]
fn test_sandbox_policy_default() {
    let policy = SandboxPolicy::default();
    assert!(!policy.has_full_network_access());
    assert!(!policy.has_full_disk_write_access());
    assert!(policy.has_full_disk_read_access());
}

#[test]
fn test_sandbox_policy_permissive() {
    let policy = SandboxPolicy::permissive();
    assert!(policy.has_full_network_access());
    assert!(policy.has_full_disk_write_access());
    assert!(policy.has_full_disk_read_access());
}

#[test]
fn test_sandbox_policy_restrictive() {
    let policy = SandboxPolicy::restrictive();
    assert!(!policy.has_full_network_access());
    assert!(!policy.has_full_disk_write_access());
    assert!(!policy.has_full_disk_read_access());
}

#[test]
fn test_sandbox_policy_builder() {
    let policy = SandboxPolicy::default()
        .with_network(NetworkAccess::UnixOnly)
        .with_disk_write(DiskAccess::Restricted)
        .add_writable_root("/tmp/test")
        .add_readable_root("/home/user");

    assert!(!policy.has_full_network_access());
    assert!(!policy.has_full_disk_write_access());
}

#[test]
fn test_writable_root() {
    let root = WritableRoot::new("/tmp/test");
    assert_eq!(root.root, PathBuf::from("/tmp/test"));
    assert!(root.recursive);

    let root = WritableRoot::non_recursive("/tmp/test2");
    assert!(!root.recursive);
}

#[test]
fn test_sandbox_config() {
    let policy = SandboxPolicy::default();
    let config = SandboxConfig::new(policy, "/home/user/project")
        .with_env("PATH", "/usr/bin")
        .without_inherit_env();

    assert!(!config.inherit_env);
    assert_eq!(config.env_vars.len(), 1);
}

#[test]
fn test_get_writable_roots_with_cwd() {
    let policy = SandboxPolicy::default()
        .with_disk_write(DiskAccess::Restricted)
        .add_writable_root("/tmp");

    let cwd = PathBuf::from("/home/user/project");
    let roots = policy.get_writable_roots_with_cwd(&cwd);

    assert_eq!(roots.len(), 2);
}

#[test]
fn test_network_access_variants() {
    assert_eq!(format!("{:?}", NetworkAccess::None), "None");
    assert_eq!(format!("{:?}", NetworkAccess::UnixOnly), "UnixOnly");
    assert_eq!(format!("{:?}", NetworkAccess::Full), "Full");
}

#[test]
fn test_disk_access_variants() {
    assert_eq!(format!("{:?}", DiskAccess::None), "None");
    assert_eq!(format!("{:?}", DiskAccess::ReadOnly), "ReadOnly");
    assert_eq!(format!("{:?}", DiskAccess::Restricted), "Restricted");
    assert_eq!(format!("{:?}", DiskAccess::Full), "Full");
}

#[test]
fn test_is_sandbox_supported() {
    #[cfg(target_os = "linux")]
    assert!(is_sandbox_supported());

    #[cfg(not(target_os = "linux"))]
    assert!(!is_sandbox_supported());
}

#[test]
fn test_apply_sandbox_policy_noop_on_permissive() {
    let policy = SandboxPolicy::permissive();
    let cwd = PathBuf::from("/tmp");
    let result = apply_sandbox_policy(&policy, &cwd);
    assert!(result.is_ok());
}
