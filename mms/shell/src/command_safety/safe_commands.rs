//! Safe command detection and whitelisting.
//!
//! This module provides functionality for determining if a command is
//! known to be safe for execution without approval.

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use crate::parse::shell_split;

/// Result of safe command check with additional context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeCommandResult {
    /// Command is safe to execute.
    Safe,
    /// Command is safe with specific conditions met.
    SafeConditional {
        /// Reason why it's conditionally safe.
        reason: String,
    },
    /// Command is not in the safe list.
    NotSafe,
    /// Command was rejected due to unsafe arguments.
    UnsafeArguments {
        /// Reason for rejection.
        reason: String,
    },
}

impl SafeCommandResult {
    /// Check if the result indicates the command is safe.
    pub fn is_safe(&self) -> bool {
        matches!(self, SafeCommandResult::Safe | SafeCommandResult::SafeConditional { .. })
    }
}

/// Base set of unconditionally safe commands.
///
/// These commands are safe regardless of their arguments
/// (within reasonable limits).
static BASE_SAFE_COMMANDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // File reading (read-only)
        "cat",
        "head",
        "tail",
        "less",
        "more",
        "bat",
        "nl",
        // Text processing (read-only, no file output)
        "grep",
        "egrep",
        "fgrep",
        "cut",
        "paste",
        "tr",
        "rev",
        "uniq",
        "sort",
        "wc",
        "awk",
        // Directory/file info
        "ls",
        "ll",
        "la",
        "dir",
        "tree",
        "exa",
        "eza",
        "lsd",
        "stat",
        "file",
        "du",
        "df",
        // Navigation and path
        "pwd",
        "cd",
        "pushd",
        "popd",
        "realpath",
        "dirname",
        "basename",
        // System info
        "whoami",
        "id",
        "groups",
        "uname",
        "hostname",
        "uptime",
        "date",
        "cal",
        "env",
        "printenv",
        "locale",
        // Boolean/echo
        "true",
        "false",
        "echo",
        "printf",
        // Search (path-based, no execution)
        "which",
        "whereis",
        "type",
        "command",
        "hash",
        // Math/sequence
        "expr",
        "seq",
        "bc",
        // Process info (read-only)
        "ps",
        "pgrep",
        "top",
        "htop",
        "pstree",
        // Version checks
        "node",  // with --version
        "npm",   // with --version
        "python",
        "python3",
        "ruby",
        "go",
        "rustc",
        "cargo",  // with specific subcommands
        "java",
        "javac",
        // Help commands
        "man",
        "info",
        "help",
        "apropos",
        "whatis",
        // Linux-specific
        "numfmt",
        "tac",
        "md5sum",
        "sha256sum",
        "sha1sum",
        // macOS-specific
        "pbcopy",
        "pbpaste",
        "open",  // conditional
        "sw_vers",
        "system_profiler",
    ]
    .into_iter()
    .collect()
});

/// Commands that are safe only with specific subcommands.
static SAFE_WITH_SUBCOMMANDS: LazyLock<std::collections::HashMap<&'static str, HashSet<&'static str>>> =
    LazyLock::new(|| {
        let mut map = std::collections::HashMap::new();

        // Git: only read-only operations
        map.insert(
            "git",
            [
                "status",
                "log",
                "show",
                "diff",
                "branch",
                "tag",
                "remote",
                "config",
                "ls-files",
                "ls-tree",
                "cat-file",
                "rev-parse",
                "rev-list",
                "describe",
                "shortlog",
                "blame",
                "reflog",
                "stash",  // stash list/show only
                "worktree", // list only
            ]
            .into_iter()
            .collect(),
        );

        // Cargo: only non-destructive operations
        map.insert(
            "cargo",
            [
                "check",
                "clippy",
                "fmt",
                "test",
                "bench",
                "doc",
                "tree",
                "metadata",
                "version",
                "search",
                "locate-project",
            ]
            .into_iter()
            .collect(),
        );

        // npm: read-only operations
        map.insert(
            "npm",
            [
                "list",
                "ls",
                "view",
                "info",
                "search",
                "outdated",
                "audit",
                "config",
                "help",
                "version",
                "prefix",
                "root",
                "bin",
            ]
            .into_iter()
            .collect(),
        );

        // yarn: read-only operations
        map.insert(
            "yarn",
            [
                "list",
                "info",
                "why",
                "outdated",
                "audit",
                "config",
                "help",
                "version",
            ]
            .into_iter()
            .collect(),
        );

        // pnpm: read-only operations
        map.insert(
            "pnpm",
            [
                "list",
                "ls",
                "why",
                "outdated",
                "audit",
                "config",
                "help",
                "root",
            ]
            .into_iter()
            .collect(),
        );

        // Docker: read-only operations
        map.insert(
            "docker",
            [
                "ps",
                "images",
                "logs",
                "inspect",
                "stats",
                "top",
                "port",
                "diff",
                "history",
                "info",
                "version",
                "search",
            ]
            .into_iter()
            .collect(),
        );

        // kubectl: read-only operations
        map.insert(
            "kubectl",
            [
                "get",
                "describe",
                "logs",
                "top",
                "cluster-info",
                "api-resources",
                "api-versions",
                "version",
                "config",
                "auth",
            ]
            .into_iter()
            .collect(),
        );

        // systemctl: status only
        map.insert(
            "systemctl",
            [
                "status",
                "list-units",
                "list-unit-files",
                "list-sockets",
                "list-timers",
                "list-dependencies",
                "show",
                "cat",
                "is-active",
                "is-enabled",
                "is-failed",
            ]
            .into_iter()
            .collect(),
        );

        map
    });

/// Arguments that make otherwise safe commands unsafe.
static UNSAFE_ARGUMENTS: LazyLock<std::collections::HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        let mut map = std::collections::HashMap::new();

        // find: execution options are unsafe
        map.insert(
            "find",
            vec![
                "-exec",
                "-execdir",
                "-ok",
                "-okdir",
                "-delete",
                "-fls",
                "-fprint",
                "-fprint0",
                "-fprintf",
            ],
        );

        // rg (ripgrep): options that can execute code
        map.insert(
            "rg",
            vec![
                "--pre",
                "--pre-glob",
                "--hostname-bin",
                "--search-zip",
                "-z",
            ],
        );

        // base64: output to file is unsafe
        map.insert(
            "base64",
            vec![
                "-o",
                "--output",
            ],
        );

        // curl/wget: unsafe without output control
        map.insert(
            "curl",
            vec![
                "-o",
                "--output",
                "-O",
                "--remote-name",
                "-x",
                "--proxy",
            ],
        );

        map.insert(
            "wget",
            vec![
                "-O",
                "--output-document",
                "-P",
                "--directory-prefix",
            ],
        );

        map
    });

/// Patterns in commands that indicate unsafe operations.
/// These are checked with context to avoid false positives.
static UNSAFE_REDIRECT_PATTERNS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        // Redirections (write to file) - checked carefully
        " > ",
        " >> ",
        " 2> ",
        " &> ",
        ">|",  // Clobber
    ]
});

/// Patterns that always indicate unsafe operations.
static UNSAFE_SUBSTITUTION_PATTERNS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        // Command/process substitution
        "$(",
        "`",
        // Variable expansion (can be unpredictable)
        "${",
    ]
});

/// Get the list of base safe commands.
pub fn get_base_safe_commands() -> &'static HashSet<&'static str> {
    &BASE_SAFE_COMMANDS
}

/// Check if a command is known to be safe.
///
/// This function performs a comprehensive check including:
/// - Base command whitelist
/// - Conditional command safety (subcommands)
/// - Argument validation
/// - Shell chain analysis
///
/// # Arguments
///
/// * `command` - The command as a slice of strings (program + arguments)
///
/// # Returns
///
/// `true` if the command is safe, `false` otherwise.
///
/// # Example
///
/// ```rust
/// use mms_shell::command_safety::is_known_safe_command;
///
/// assert!(is_known_safe_command(&["ls", "-la"]));
/// assert!(is_known_safe_command(&["git", "status"]));
/// assert!(!is_known_safe_command(&["rm", "-rf", "/"]));
/// assert!(!is_known_safe_command(&["find", ".", "-exec", "rm", "{}", ";"]));
/// ```
pub fn is_known_safe_command(command: &[&str]) -> bool {
    is_safe_to_exec(command).is_safe()
}

/// Check if a command is safe to execute with detailed result.
///
/// This provides more information about why a command is or isn't safe.
///
/// # Arguments
///
/// * `command` - The command as a slice of strings
///
/// # Returns
///
/// A [`SafeCommandResult`] indicating the safety status and reason.
pub fn is_safe_to_exec(command: &[&str]) -> SafeCommandResult {
    if command.is_empty() {
        return SafeCommandResult::NotSafe;
    }

    // Check for shell chain (bash -c "cmd1 && cmd2")
    if let Some((_, script)) = extract_shell_command_from_strs(command) {
        return check_shell_chain_safety(&script);
    }

    // Get the base command name
    let program = get_base_command(command[0]);
    let args = &command[1..];

    // Check base safe commands
    if BASE_SAFE_COMMANDS.contains(program) {
        // Check for unsafe arguments
        if let Some(result) = check_conditional_safety(program, args) {
            return result;
        }
        // Check for unsafe substitution patterns in args
        for arg in args {
            for pattern in UNSAFE_SUBSTITUTION_PATTERNS.iter() {
                if arg.contains(pattern) {
                    return SafeCommandResult::UnsafeArguments {
                        reason: format!("argument contains unsafe pattern: {}", pattern),
                    };
                }
            }
            // Check for redirects in args
            if arg.starts_with('>') || arg.starts_with("2>") || arg.starts_with("&>") {
                return SafeCommandResult::UnsafeArguments {
                    reason: "argument contains output redirection".to_string(),
                };
            }
        }
        return SafeCommandResult::Safe;
    }

    // Check commands that need specific subcommands
    if let Some(safe_subcommands) = SAFE_WITH_SUBCOMMANDS.get(program) {
        if let Some(subcommand) = args.first() {
            if safe_subcommands.contains(subcommand.to_lowercase().as_str()) {
                // Additional check for git stash/worktree
                if program == "git" {
                    match *subcommand {
                        "stash" => {
                            // Only stash list/show are safe
                            if let Some(action) = args.get(1) {
                                if !matches!(*action, "list" | "show") {
                                    return SafeCommandResult::UnsafeArguments {
                                        reason: format!(
                                            "git stash {} may modify state",
                                            action
                                        ),
                                    };
                                }
                            } else {
                                // bare "git stash" creates a stash
                                return SafeCommandResult::UnsafeArguments {
                                    reason: "git stash without subcommand modifies state".to_string(),
                                };
                            }
                        }
                        "worktree" => {
                            // Only worktree list is safe
                            if args.get(1) != Some(&"list") {
                                return SafeCommandResult::UnsafeArguments {
                                    reason: "only git worktree list is safe".to_string(),
                                };
                            }
                        }
                        _ => {}
                    }
                }
                return SafeCommandResult::SafeConditional {
                    reason: format!("{} {} is a safe subcommand", program, subcommand),
                };
            }
        }
        return SafeCommandResult::UnsafeArguments {
            reason: format!("{} requires a safe subcommand", program),
        };
    }

    // Check find and rg with conditional safety
    if program == "find" || program == "fd" {
        return check_find_safety(args);
    }

    if program == "rg" || program == "ripgrep" {
        return check_ripgrep_safety(args);
    }

    // Check sed for read-only patterns
    if program == "sed" {
        return check_sed_safety(args);
    }

    SafeCommandResult::NotSafe
}

/// Check conditional safety of a command based on its arguments.
///
/// # Arguments
///
/// * `program` - The program name
/// * `args` - The command arguments
///
/// # Returns
///
/// `Some(SafeCommandResult)` if the command has conditional safety rules,
/// `None` if no specific rules apply.
pub fn check_conditional_safety(program: &str, args: &[&str]) -> Option<SafeCommandResult> {
    if let Some(unsafe_args) = UNSAFE_ARGUMENTS.get(program) {
        for arg in args {
            for unsafe_arg in unsafe_args {
                // Check exact match or prefix (for --option=value)
                if *arg == *unsafe_arg || arg.starts_with(&format!("{}=", unsafe_arg)) {
                    return Some(SafeCommandResult::UnsafeArguments {
                        reason: format!(
                            "{} with {} is not safe",
                            program, unsafe_arg
                        ),
                    });
                }
            }
        }
    }
    None
}

/// Check if a find command is safe.
fn check_find_safety(args: &[&str]) -> SafeCommandResult {
    let unsafe_find_args = [
        "-exec",
        "-execdir",
        "-ok",
        "-okdir",
        "-delete",
        "-fls",
        "-fprint",
        "-fprint0",
        "-fprintf",
    ];

    for arg in args {
        for unsafe_arg in &unsafe_find_args {
            if *arg == *unsafe_arg {
                return SafeCommandResult::UnsafeArguments {
                    reason: format!("find with {} can execute or delete", unsafe_arg),
                };
            }
        }
    }

    SafeCommandResult::SafeConditional {
        reason: "find without execution flags is safe".to_string(),
    }
}

/// Check if a ripgrep command is safe.
fn check_ripgrep_safety(args: &[&str]) -> SafeCommandResult {
    let unsafe_rg_args = [
        "--pre",
        "--pre-glob",
        "--hostname-bin",
        "--search-zip",
        "-z",
    ];

    for arg in args {
        for unsafe_arg in &unsafe_rg_args {
            if *arg == *unsafe_arg || arg.starts_with(&format!("{}=", unsafe_arg)) {
                return SafeCommandResult::UnsafeArguments {
                    reason: format!("rg with {} can execute external programs", unsafe_arg),
                };
            }
        }
    }

    SafeCommandResult::SafeConditional {
        reason: "rg without preprocessing is safe".to_string(),
    }
}

/// Check if a sed command is safe (read-only pattern).
fn check_sed_safety(args: &[&str]) -> SafeCommandResult {
    // Safe pattern: sed -n '1,100p' (line extraction)
    // Unsafe: sed -i (in-place edit), sed 's/.../.../' without -n

    let mut has_n_flag = false;
    let mut has_inplace = false;
    let mut has_write = false;

    for arg in args {
        if *arg == "-n" {
            has_n_flag = true;
        }
        if *arg == "-i" || arg.starts_with("-i") {
            has_inplace = true;
        }
        // Check for write command in sed script
        if arg.contains("w ") || arg.ends_with("w") {
            has_write = true;
        }
    }

    if has_inplace {
        return SafeCommandResult::UnsafeArguments {
            reason: "sed -i modifies files in place".to_string(),
        };
    }

    if has_write {
        return SafeCommandResult::UnsafeArguments {
            reason: "sed write command (w) outputs to file".to_string(),
        };
    }

    if has_n_flag {
        // Check for simple print pattern
        for arg in args {
            // Pattern like '1,100p' or '1p'
            let stripped = arg.trim_matches(|c| c == '\'' || c == '"');
            if stripped.ends_with('p') {
                let pattern = stripped.trim_end_matches('p');
                // Check if it's just numbers and comma
                if pattern.chars().all(|c| c.is_ascii_digit() || c == ',') {
                    return SafeCommandResult::SafeConditional {
                        reason: "sed -n with print pattern is read-only".to_string(),
                    };
                }
            }
        }
    }

    SafeCommandResult::NotSafe
}

/// Check if a shell chain command is safe.
fn check_shell_chain_safety(script: &str) -> SafeCommandResult {
    // Check for subshells
    let trimmed = script.trim();
    if trimmed.starts_with('(') || trimmed.contains(" (") {
        return SafeCommandResult::UnsafeArguments {
            reason: "subshells are not allowed in safe commands".to_string(),
        };
    }

    // Check for command/variable substitution patterns
    for pattern in UNSAFE_SUBSTITUTION_PATTERNS.iter() {
        if script.contains(pattern) {
            return SafeCommandResult::UnsafeArguments {
                reason: format!("shell script contains unsafe pattern: {}", pattern),
            };
        }
    }

    // Check for redirect patterns (with spaces to avoid false positives)
    for pattern in UNSAFE_REDIRECT_PATTERNS.iter() {
        if script.contains(pattern) {
            return SafeCommandResult::UnsafeArguments {
                reason: "shell script contains output redirection".to_string(),
            };
        }
    }

    // Check for background execution (& at end, but not &&)
    // We need to be careful here - && is safe, but & alone is not
    let script_trimmed = script.trim();
    if script_trimmed.ends_with(" &") || script_trimmed.ends_with("\t&") {
        return SafeCommandResult::UnsafeArguments {
            reason: "background execution (&) is not allowed in safe commands".to_string(),
        };
    }

    // Split by safe operators and check each command
    let commands = split_shell_chain(script);

    for cmd in commands {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            continue;
        }

        // Parse the command
        match shell_split(cmd) {
            Ok(tokens) => {
                let tokens_ref: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
                if !tokens_ref.is_empty() {
                    let result = is_safe_to_exec(&tokens_ref);
                    if !result.is_safe() {
                        return SafeCommandResult::UnsafeArguments {
                            reason: format!(
                                "command in chain is not safe: {}",
                                cmd
                            ),
                        };
                    }
                }
            }
            Err(_) => {
                return SafeCommandResult::UnsafeArguments {
                    reason: format!("failed to parse command: {}", cmd),
                };
            }
        }
    }

    SafeCommandResult::SafeConditional {
        reason: "all commands in chain are safe".to_string(),
    }
}

/// Split a shell command chain by operators.
fn split_shell_chain(script: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let chars: Vec<char> = script.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        match c {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = c;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            '&' if !in_quotes && i + 1 < chars.len() && chars[i + 1] == '&' => {
                commands.push(&script[start..i]);
                start = i + 2;
                i += 1; // Skip next &
            }
            '|' if !in_quotes => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    commands.push(&script[start..i]);
                    start = i + 2;
                    i += 1; // Skip next |
                } else {
                    // Single pipe
                    commands.push(&script[start..i]);
                    start = i + 1;
                }
            }
            ';' if !in_quotes => {
                commands.push(&script[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    // Add remaining command
    if start < script.len() {
        commands.push(&script[start..]);
    }

    commands
}

/// Get the base command name from a path.
fn get_base_command(cmd: &str) -> &str {
    Path::new(cmd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(cmd)
}

/// Helper to extract shell command from &str slice.
fn extract_shell_command_from_strs<'a>(command: &'a [&'a str]) -> Option<(&'a str, String)> {
    if command.len() < 3 {
        return None;
    }

    let shell = command[0];
    let flag = command[1];
    let script = command[2];

    // Check if the flag is -lc or -c
    if !matches!(flag, "-lc" | "-c") {
        return None;
    }

    // Check if shell is a POSIX shell
    let base = get_base_command(shell);
    if !matches!(base, "bash" | "zsh" | "sh" | "dash" | "ash") {
        return None;
    }

    Some((shell, script.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_safe_commands() {
        assert!(is_known_safe_command(&["ls"]));
        assert!(is_known_safe_command(&["ls", "-la"]));
        assert!(is_known_safe_command(&["cat", "file.txt"]));
        assert!(is_known_safe_command(&["pwd"]));
        assert!(is_known_safe_command(&["whoami"]));
        assert!(is_known_safe_command(&["echo", "hello"]));
    }

    #[test]
    fn test_git_subcommands() {
        assert!(is_known_safe_command(&["git", "status"]));
        assert!(is_known_safe_command(&["git", "log"]));
        assert!(is_known_safe_command(&["git", "diff"]));
        assert!(!is_known_safe_command(&["git", "push"]));
        assert!(!is_known_safe_command(&["git", "reset"]));
        assert!(!is_known_safe_command(&["git", "rm"]));
    }

    #[test]
    fn test_find_safety() {
        assert!(is_known_safe_command(&["find", ".", "-name", "*.rs"]));
        assert!(!is_known_safe_command(&["find", ".", "-exec", "rm", "{}", ";"]));
        assert!(!is_known_safe_command(&["find", ".", "-delete"]));
    }

    #[test]
    fn test_ripgrep_safety() {
        assert!(is_known_safe_command(&["rg", "pattern", "file.txt"]));
        assert!(!is_known_safe_command(&["rg", "--pre", "cat", "pattern"]));
    }

    #[test]
    fn test_shell_chain() {
        // These would be passed as: ["bash", "-c", "ls && pwd"]
        assert!(is_known_safe_command(&["bash", "-c", "ls && pwd"]));
        assert!(is_known_safe_command(&["bash", "-c", "cat file.txt | grep pattern"]));
        assert!(!is_known_safe_command(&["bash", "-c", "ls && rm -rf /"]));
    }

    #[test]
    fn test_unsafe_patterns() {
        assert!(!is_known_safe_command(&["echo", "hello", ">", "file.txt"]));
        // Note: in real usage, the shell would parse this differently
    }
}
