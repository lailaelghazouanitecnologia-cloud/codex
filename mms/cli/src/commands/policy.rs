use anyhow::Result;
use mms_execpolicy::{load_policy, default_heuristics, Decision};
use std::path::PathBuf;

/// Validate a policy file
pub async fn policy_validate(path: PathBuf) -> Result<()> {
    println!("Validating policy file: {}", path.display());
    println!();

    match load_policy(&path) {
        Ok(policy) => {
            println!("✓ Policy file is valid");
            println!();
            println!("Policy Statistics:");
            println!("  Rules: {}", policy.rule_count());
            Ok(())
        }
        Err(e) => {
            println!("✗ Policy file is invalid");
            println!();
            println!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Test a command against a policy
pub async fn policy_test(
    policy_path: Option<PathBuf>,
    command: String,
) -> Result<()> {
    // Parse the command string into parts
    let parts: Vec<String> = shell_words::split(&command)
        .unwrap_or_else(|_| vec![command.clone()]);

    if parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }

    println!("Testing command: {}", parts.join(" "));
    println!();

    let decision = if let Some(path) = policy_path {
        println!("Using policy file: {}", path.display());
        let policy = load_policy(&path)?;
        let eval = policy.check(&parts);
        println!("Matched rules: {}", eval.matched_rules.len());
        for rule in &eval.matched_rules {
            println!("  - {:?}", rule);
        }
        eval.decision
    } else {
        println!("Using default heuristics (no policy file)");
        default_heuristics(&parts)
    };

    println!();
    match decision {
        Decision::Allow => {
            println!("Decision: ✓ ALLOW");
            println!("  Command will execute without prompting");
        }
        Decision::Prompt => {
            println!("Decision: ⚠ PROMPT");
            println!("  Command will require user approval");
        }
        Decision::Forbidden => {
            println!("Decision: ✗ FORBIDDEN");
            println!("  Command will be rejected");
        }
    }

    Ok(())
}

/// Show default heuristics information
pub async fn policy_info() -> Result<()> {
    println!("Execution Policy Information");
    println!("============================");
    println!();
    println!("Default Behavior:");
    println!("  - Unknown commands: PROMPT (require approval)");
    println!("  - Empty commands: FORBIDDEN");
    println!();
    println!("Safe Programs (ALLOW):");
    let safe = [
        "ls", "cat", "head", "tail", "less", "more",
        "grep", "rg", "ag", "ack",
        "find", "fd", "locate",
        "wc", "sort", "uniq", "cut", "tr",
        "echo", "printf", "date", "cal",
        "pwd", "whoami", "hostname",
        "file", "stat", "du", "df",
        "diff", "cmp", "comm",
        "jq", "yq", "xq",
        "git", "hg", "svn",
        "node", "npm", "npx", "yarn", "pnpm",
        "python", "python3", "pip", "pip3",
        "cargo", "rustc", "rustfmt", "clippy",
        "go", "gofmt",
        "make", "cmake", "ninja",
        "sh", "bash", "zsh", "fish",
    ];
    for chunk in safe.chunks(8) {
        println!("  {}", chunk.join(", "));
    }
    println!();
    println!("Dangerous Programs (PROMPT):");
    let dangerous = [
        "rm", "rmdir", "mv", "chmod", "chown",
        "sudo", "su", "pkill", "kill", "killall",
        "dd", "mkfs", "fdisk", "parted",
        "iptables", "ip6tables", "nft",
        "systemctl", "service",
        "reboot", "shutdown", "halt", "poweroff",
        "curl", "wget", "nc", "ncat", "netcat",
        "ssh", "scp", "rsync",
    ];
    for chunk in dangerous.chunks(6) {
        println!("  {}", chunk.join(", "));
    }
    println!();
    println!("Shell Command Unwrapping:");
    println!("  - Commands like 'sh -c \"ls -la\"' are analyzed");
    println!("  - The actual command (ls) is evaluated, not the shell");
    println!();
    println!("Policy File Format (TOML):");
    println!("  [policy]");
    println!("  default = \"prompt\"  # allow, prompt, or forbidden");
    println!();
    println!("  [[rules]]");
    println!("  type = \"prefix\"");
    println!("  command = [\"git\", \"status\"]");
    println!("  decision = \"allow\"");
    println!();
    println!("  [[rules]]");
    println!("  type = \"glob\"");
    println!("  program = \"npm\"");
    println!("  decision = \"allow\"");

    Ok(())
}
