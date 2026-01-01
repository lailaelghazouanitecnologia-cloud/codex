use anyhow::Result;
use mms_exec::{CommandRequest, SandboxedRunner};
use mms_execpolicy::load_policy;
use mms_linux_sandbox::{NetworkAccess, DiskAccess, SandboxPolicy};
use std::path::PathBuf;

/// Execute a shell command with optional sandbox
pub async fn shell_exec(
    command: String,
    policy_path: Option<PathBuf>,
    no_sandbox: bool,
    allow_network: bool,
    allow_write: bool,
    timeout_ms: u64,
) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Parse command into parts
    let parts: Vec<String> = shell_words::split(&command)
        .unwrap_or_else(|_| vec![command.clone()]);

    if parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }

    // Build sandbox policy
    let sandbox_policy = if no_sandbox {
        SandboxPolicy::permissive()
    } else {
        let mut policy = SandboxPolicy::default()
            .with_disk_read(DiskAccess::Full)
            .add_writable_root(&cwd);

        if allow_network {
            policy = policy.with_network(NetworkAccess::Full);
        } else {
            policy = policy.with_network(NetworkAccess::None);
        }

        if allow_write {
            policy = policy.with_disk_write(DiskAccess::Full);
        } else {
            policy = policy.with_disk_write(DiskAccess::Restricted);
        }

        policy
    };

    // Create runner
    let mut runner = SandboxedRunner::new(cwd)
        .with_sandbox_policy(sandbox_policy)
        .with_default_timeout(timeout_ms);

    // Load exec policy if provided
    if let Some(path) = &policy_path {
        let exec_policy = load_policy(path)?;
        runner = runner.with_exec_policy(exec_policy);
    }

    // Build command request
    let program = parts[0].clone();
    let args: Vec<String> = parts[1..].to_vec();

    let request = CommandRequest::new(&program)
        .with_args(args);

    // Print execution info
    println!("Executing: {}", command);
    if !no_sandbox {
        println!("Sandbox: enabled");
        if allow_network {
            println!("  Network: allowed");
        } else {
            println!("  Network: blocked");
        }
        if allow_write {
            println!("  Disk write: full");
        } else {
            println!("  Disk write: restricted");
        }
    } else {
        println!("Sandbox: disabled");
    }
    if let Some(path) = &policy_path {
        println!("Policy: {}", path.display());
    }
    println!("Timeout: {}ms", timeout_ms);
    println!();
    println!("--- Output ---");

    // Execute
    match runner.run(request).await {
        Ok(output) => {
            // Print stdout
            if !output.stdout.is_empty() {
                print!("{}", output.stdout);
            }

            // Print stderr to stderr
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }

            println!();
            println!("--- Result ---");
            if output.success() {
                println!("Exit code: 0 (success)");
            } else {
                println!("Exit code: {} (failure)", output.exit_code);
            }

            if output.sandboxed {
                println!("Sandboxed: yes");
            } else {
                println!("Sandboxed: no");
            }

            if !output.success() {
                std::process::exit(output.exit_code);
            }

            Ok(())
        }
        Err(e) => {
            eprintln!();
            eprintln!("--- Error ---");
            eprintln!("Execution failed: {}", e);
            std::process::exit(1);
        }
    }
}
