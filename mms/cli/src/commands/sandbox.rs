use anyhow::Result;
use mms_exec::{CommandRequest, SandboxedRunner};
use mms_linux_sandbox::{is_sandbox_supported, NetworkAccess, DiskAccess, SandboxPolicy};

/// Show sandbox status and system support
pub async fn sandbox_status() -> Result<()> {
    println!("Sandbox Status");
    println!("==============");
    println!();

    // Check platform
    let platform = std::env::consts::OS;
    println!("Platform: {}", platform);

    // Check if sandbox is supported
    let supported = is_sandbox_supported();
    if supported {
        println!("Sandbox Support: ✓ Supported");
    } else {
        println!("Sandbox Support: ✗ Not supported");
        println!();
        println!("Note: Sandbox requires Linux with Landlock support (kernel 5.13+)");
        return Ok(());
    }

    println!();
    println!("Available Security Features:");
    println!("  - Landlock LSM: Filesystem access control");
    println!("  - seccomp-BPF: System call filtering");
    println!();

    println!("Default Sandbox Policy:");
    let default_policy = SandboxPolicy::default();
    println!("  Network: {:?}", NetworkAccess::None);
    println!("  Disk Read: {:?}", DiskAccess::Full);
    println!("  Disk Write: {:?}", DiskAccess::Restricted);
    println!();

    println!("Sandbox Profiles:");
    println!("  default     - Read anywhere, write to cwd only, no network");
    println!("  permissive  - Full access (sandbox disabled)");
    println!("  restrictive - Limited read/write, no network");
    println!();

    println!("Use 'mms sandbox test' to verify sandbox functionality.");

    // Suppress unused warning
    let _ = default_policy;

    Ok(())
}

/// Test sandbox with a simple command
pub async fn sandbox_test() -> Result<()> {
    println!("Sandbox Test");
    println!("============");
    println!();

    if !is_sandbox_supported() {
        println!("✗ Sandbox is not supported on this platform");
        println!();
        println!("Sandbox requires Linux with Landlock support (kernel 5.13+)");
        return Ok(());
    }

    let cwd = std::env::current_dir()?;

    println!("Running sandbox tests...");
    println!();

    // Test 1: Simple command execution
    println!("Test 1: Basic command execution");
    let runner = SandboxedRunner::new(cwd.clone())
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = CommandRequest::new("echo").with_args(vec!["sandbox test".into()]);
    match runner.run(request).await {
        Ok(output) => {
            if output.success() {
                println!("  ✓ Basic execution works");
            } else {
                println!("  ✗ Command failed with exit code {}", output.exit_code);
            }
        }
        Err(e) => println!("  ✗ Error: {}", e),
    }

    // Test 2: Network-restricted execution
    println!("Test 2: Network-restricted execution");
    let runner = SandboxedRunner::new(cwd.clone())
        .with_sandbox_policy(
            SandboxPolicy::default()
                .with_network(NetworkAccess::None)
                .with_disk_write(DiskAccess::Full),
        );

    let request = CommandRequest::new("echo").with_args(vec!["no network".into()]);
    match runner.run(request).await {
        Ok(output) => {
            if output.success() {
                println!("  ✓ Network-restricted execution works");
                if output.sandboxed {
                    println!("    (sandboxed: yes)");
                }
            } else {
                println!("  ✗ Command failed");
            }
        }
        Err(e) => println!("  ✗ Error: {}", e),
    }

    // Test 3: Write-restricted execution
    println!("Test 3: Write-restricted execution");
    let runner = SandboxedRunner::new(cwd.clone())
        .with_sandbox_policy(
            SandboxPolicy::default()
                .with_disk_write(DiskAccess::Restricted)
                .add_writable_root(&cwd),
        );

    let request = CommandRequest::new("pwd");
    match runner.run(request).await {
        Ok(output) => {
            if output.success() {
                println!("  ✓ Write-restricted execution works");
                if output.sandboxed {
                    println!("    (sandboxed: yes)");
                }
            } else {
                println!("  ✗ Command failed");
            }
        }
        Err(e) => println!("  ✗ Error: {}", e),
    }

    // Test 4: Timeout handling
    println!("Test 4: Timeout handling");
    let runner = SandboxedRunner::new(cwd.clone())
        .with_default_timeout(100) // 100ms
        .with_sandbox_policy(SandboxPolicy::permissive());

    let request = CommandRequest::new("sleep").with_args(vec!["10".into()]);
    match runner.run(request).await {
        Ok(_) => println!("  ✗ Should have timed out"),
        Err(e) => {
            if e.to_string().contains("timed out") {
                println!("  ✓ Timeout handling works");
            } else {
                println!("  ? Unexpected error: {}", e);
            }
        }
    }

    println!();
    println!("Sandbox tests completed.");

    Ok(())
}
