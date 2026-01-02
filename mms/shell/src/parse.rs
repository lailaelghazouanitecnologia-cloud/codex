//! Shell command parsing.
//!
//! This module provides functionality for parsing shell commands,
//! extracting structured information, and classifying command types.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{ShellType, detect_shell_type};

/// Errors that can occur during command parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Invalid command format.
    #[error("invalid command format: {0}")]
    InvalidFormat(String),

    /// Shell tokenization failed.
    #[error("tokenization failed: {0}")]
    TokenizationFailed(String),

    /// Unsupported construct.
    #[error("unsupported shell construct: {0}")]
    UnsupportedConstruct(String),
}

/// Result type for parsing operations.
pub type ParseResult<T> = Result<T, ParseError>;

/// Parsed command classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParsedCommand {
    /// File read operation.
    Read {
        /// The full command string.
        cmd: String,
        /// Target file path.
        path: PathBuf,
        /// Optional line range (start, end).
        lines: Option<(usize, usize)>,
    },

    /// Search operation (grep, rg, find).
    Search {
        /// The full command string.
        cmd: String,
        /// Search pattern/query.
        query: Option<String>,
        /// Search path.
        path: Option<PathBuf>,
    },

    /// File listing operation.
    ListFiles {
        /// The full command string.
        cmd: String,
        /// Target directory.
        path: Option<PathBuf>,
        /// Whether to list recursively.
        recursive: bool,
    },

    /// Directory change operation.
    ChangeDir {
        /// The full command string.
        cmd: String,
        /// Target directory.
        path: PathBuf,
    },

    /// Write/edit file operation.
    Write {
        /// The full command string.
        cmd: String,
        /// Target file path.
        path: PathBuf,
    },

    /// Unknown/unclassified command.
    Unknown {
        /// The full command string.
        cmd: String,
    },
}

impl ParsedCommand {
    /// Get the command string.
    pub fn cmd(&self) -> &str {
        match self {
            ParsedCommand::Read { cmd, .. } => cmd,
            ParsedCommand::Search { cmd, .. } => cmd,
            ParsedCommand::ListFiles { cmd, .. } => cmd,
            ParsedCommand::ChangeDir { cmd, .. } => cmd,
            ParsedCommand::Write { cmd, .. } => cmd,
            ParsedCommand::Unknown { cmd } => cmd,
        }
    }

    /// Get a human-readable description of the command.
    pub fn description(&self) -> String {
        match self {
            ParsedCommand::Read { path, lines, .. } => {
                let path_str = path.display();
                match lines {
                    Some((start, end)) => format!("Read lines {}-{} of {}", start, end, path_str),
                    None => format!("Read {}", path_str),
                }
            }
            ParsedCommand::Search { query, path, .. } => {
                let query_str = query.as_deref().unwrap_or("*");
                match path {
                    Some(p) => format!("Search '{}' in {}", query_str, p.display()),
                    None => format!("Search '{}'", query_str),
                }
            }
            ParsedCommand::ListFiles { path, recursive, .. } => {
                let path_str = path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| ".".to_string());
                if *recursive {
                    format!("List files recursively in {}", path_str)
                } else {
                    format!("List files in {}", path_str)
                }
            }
            ParsedCommand::ChangeDir { path, .. } => {
                format!("Change directory to {}", path.display())
            }
            ParsedCommand::Write { path, .. } => {
                format!("Write to {}", path.display())
            }
            ParsedCommand::Unknown { cmd } => {
                format!("Run: {}", cmd)
            }
        }
    }
}

/// Join shell tokens into a command string.
pub fn shell_join(tokens: &[String]) -> String {
    shell_words::join(tokens.iter().map(|s| s.as_str()))
}

/// Split a shell command string into tokens.
pub fn shell_split(command: &str) -> ParseResult<Vec<String>> {
    shell_words::split(command).map_err(|e| ParseError::TokenizationFailed(e.to_string()))
}

/// Extract shell and script from a bash-style command.
///
/// For commands like `["bash", "-lc", "some script"]`, returns `Some(("bash", "some script"))`.
pub fn extract_bash_command(command: &[String]) -> Option<(&str, &str)> {
    if command.len() != 3 {
        return None;
    }

    let shell = &command[0];
    let flag = &command[1];
    let script = &command[2];

    // Check if the flag is -lc or -c
    if !matches!(flag.as_str(), "-lc" | "-c") {
        return None;
    }

    // Check if shell is a POSIX shell
    let shell_type = detect_shell_type(&PathBuf::from(shell))?;
    if !matches!(shell_type, ShellType::Bash | ShellType::Zsh | ShellType::Sh) {
        return None;
    }

    Some((shell, script))
}

/// Extract shell and script from a PowerShell-style command.
///
/// For commands like `["powershell", "-Command", "some script"]`,
/// returns `Some(("powershell", "some script"))`.
pub fn extract_powershell_command(command: &[String]) -> Option<(&str, &str)> {
    if command.len() < 3 {
        return None;
    }

    let shell = &command[0];

    // Check if it's PowerShell
    let shell_type = detect_shell_type(&PathBuf::from(shell))?;
    if !matches!(shell_type, ShellType::PowerShell) {
        return None;
    }

    // Find -Command flag (case insensitive)
    for (i, arg) in command.iter().enumerate().skip(1) {
        if arg.eq_ignore_ascii_case("-command") || arg.eq_ignore_ascii_case("-c") {
            if i + 1 < command.len() {
                return Some((shell, &command[i + 1]));
            }
        }
    }

    None
}

/// Extract shell and script from any recognized shell command.
pub fn extract_shell_command(command: &[String]) -> Option<(&str, &str)> {
    extract_bash_command(command).or_else(|| extract_powershell_command(command))
}

/// Parse a command and extract structured information.
pub fn parse_command(command: &[String]) -> Vec<ParsedCommand> {
    let parsed = parse_command_impl(command);

    // Deduplicate consecutive identical commands
    let mut deduped: Vec<ParsedCommand> = Vec::with_capacity(parsed.len());
    for cmd in parsed {
        if deduped.last() != Some(&cmd) {
            deduped.push(cmd);
        }
    }

    deduped
}

/// Internal implementation of command parsing.
fn parse_command_impl(command: &[String]) -> Vec<ParsedCommand> {
    if command.is_empty() {
        return vec![];
    }

    // Try to extract shell script
    if let Some((_, script)) = extract_shell_command(command) {
        return parse_script(script);
    }

    // Parse as a direct command
    parse_single_command(command)
}

/// Parse a shell script (may contain multiple commands).
fn parse_script(script: &str) -> Vec<ParsedCommand> {
    // Split on command separators: &&, ||, ;, |
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut prev_char = ' ';

    for c in script.chars() {
        match c {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = c;
                current.push(c);
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
                current.push(c);
            }
            '&' | '|' | ';' if !in_quotes => {
                // Check for && or ||
                if (c == '&' && prev_char == '&') || (c == '|' && prev_char == '|') {
                    // Already handled
                } else if c == '&' || c == '|' {
                    // Wait for next char
                } else {
                    // Semicolon
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        if let Ok(tokens) = shell_split(&trimmed) {
                            commands.extend(parse_single_command(&tokens));
                        }
                    }
                    current.clear();
                }
            }
            _ if !in_quotes && (prev_char == '&' || prev_char == '|') => {
                // End of separator
                let trimmed = current.trim().to_string();
                // Remove trailing & or | from current
                let trimmed = trimmed.trim_end_matches(|c| c == '&' || c == '|').trim();
                if !trimmed.is_empty() {
                    if let Ok(tokens) = shell_split(trimmed) {
                        commands.extend(parse_single_command(&tokens));
                    }
                }
                current.clear();
                current.push(c);
            }
            _ => {
                current.push(c);
            }
        }
        prev_char = c;
    }

    // Handle remaining command
    let trimmed = current.trim().trim_end_matches(|c| c == '&' || c == '|').trim();
    if !trimmed.is_empty() {
        if let Ok(tokens) = shell_split(trimmed) {
            commands.extend(parse_single_command(&tokens));
        }
    }

    if commands.is_empty() {
        vec![ParsedCommand::Unknown {
            cmd: script.to_string(),
        }]
    } else {
        commands
    }
}

/// Parse a single command (no separators).
fn parse_single_command(tokens: &[String]) -> Vec<ParsedCommand> {
    if tokens.is_empty() {
        return vec![];
    }

    let program = &tokens[0];
    let args = &tokens[1..];
    let cmd = shell_join(tokens);

    // Identify command type
    let base_program = PathBuf::from(program)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| program.clone());

    match base_program.as_str() {
        // File reading commands
        "cat" | "less" | "more" | "head" | "tail" | "bat" => {
            let path = args.iter()
                .find(|a| !a.starts_with('-'))
                .map(|p| PathBuf::from(p));

            if let Some(path) = path {
                let lines = extract_line_range(args);
                vec![ParsedCommand::Read { cmd, path, lines }]
            } else {
                vec![ParsedCommand::Unknown { cmd }]
            }
        }

        // Sed with line printing
        "sed" => {
            // Check for sed -n '1,100p' style
            if let Some(path) = args.last().filter(|a| !a.starts_with('-') && !a.contains('p')) {
                let lines = extract_sed_line_range(args);
                vec![ParsedCommand::Read {
                    cmd,
                    path: PathBuf::from(path),
                    lines,
                }]
            } else {
                vec![ParsedCommand::Unknown { cmd }]
            }
        }

        // Search commands
        "grep" | "rg" | "ag" | "ack" | "find" | "fd" => {
            let (query, path) = extract_search_args(&base_program, args);
            vec![ParsedCommand::Search { cmd, query, path }]
        }

        // File listing
        "ls" | "ll" | "la" | "tree" | "exa" | "eza" => {
            let path = args.iter()
                .find(|a| !a.starts_with('-'))
                .map(|p| PathBuf::from(p));
            let recursive = args.iter().any(|a| a == "-R" || a == "--recursive" || base_program == "tree");
            vec![ParsedCommand::ListFiles { cmd, path, recursive }]
        }

        // Directory change
        "cd" => {
            if let Some(path) = args.first() {
                vec![ParsedCommand::ChangeDir {
                    cmd,
                    path: PathBuf::from(path),
                }]
            } else {
                vec![ParsedCommand::ChangeDir {
                    cmd,
                    path: PathBuf::from("~"),
                }]
            }
        }

        // Write operations
        "echo" | "printf" if args.iter().any(|a| a.contains('>')) => {
            // Check for redirection
            if let Some(pos) = args.iter().position(|a| a.contains('>')) {
                // First try to get the next argument as the path
                let path_str = if let Some(next) = args.get(pos + 1) {
                    Some(next.as_str())
                } else {
                    // Or extract from the redirect operator itself (echo foo>file)
                    args.get(pos).and_then(|a| a.split('>').last().filter(|s| !s.is_empty()))
                };

                if let Some(p) = path_str {
                    let path = PathBuf::from(p.trim());
                    return vec![ParsedCommand::Write { cmd, path }];
                }
            }
            vec![ParsedCommand::Unknown { cmd }]
        }

        "tee" => {
            if let Some(path) = args.iter().find(|a| !a.starts_with('-')) {
                vec![ParsedCommand::Write {
                    cmd,
                    path: PathBuf::from(path),
                }]
            } else {
                vec![ParsedCommand::Unknown { cmd }]
            }
        }

        // Everything else
        _ => vec![ParsedCommand::Unknown { cmd }],
    }
}

/// Extract line range from head/tail style arguments.
fn extract_line_range(args: &[String]) -> Option<(usize, usize)> {
    for (i, arg) in args.iter().enumerate() {
        // head -n 100 or tail -n 100
        if arg == "-n" {
            if let Some(count) = args.get(i + 1).and_then(|s| s.parse::<usize>().ok()) {
                return Some((1, count));
            }
        }
        // head -100 or tail -100
        if arg.starts_with('-') && !arg.starts_with("--") {
            if let Ok(count) = arg[1..].parse::<usize>() {
                return Some((1, count));
            }
        }
    }
    None
}

/// Extract line range from sed -n '1,100p' style arguments.
fn extract_sed_line_range(args: &[String]) -> Option<(usize, usize)> {
    for arg in args {
        // Match patterns like '1,100p' or "1,100p"
        let pattern = arg.trim_matches(|c| c == '\'' || c == '"');
        if pattern.ends_with('p') {
            let range = pattern.trim_end_matches('p');
            if let Some((start, end)) = range.split_once(',') {
                if let (Ok(s), Ok(e)) = (start.parse::<usize>(), end.parse::<usize>()) {
                    return Some((s, e));
                }
            }
        }
    }
    None
}

/// Extract search query and path from search command arguments.
fn extract_search_args(program: &str, args: &[String]) -> (Option<String>, Option<PathBuf>) {
    let mut query = None;
    let mut path = None;

    match program {
        "grep" | "rg" | "ag" | "ack" => {
            // Pattern is typically the first non-flag argument
            let mut found_pattern = false;
            for arg in args {
                if arg.starts_with('-') {
                    continue;
                }
                if !found_pattern {
                    query = Some(arg.clone());
                    found_pattern = true;
                } else {
                    path = Some(PathBuf::from(arg));
                    break;
                }
            }
        }
        "find" | "fd" => {
            // For find, pattern is often after -name
            for (i, arg) in args.iter().enumerate() {
                if arg == "-name" || arg == "-iname" {
                    query = args.get(i + 1).cloned();
                } else if !arg.starts_with('-') && path.is_none() {
                    path = Some(PathBuf::from(arg));
                }
            }
        }
        _ => {}
    }

    (query, path)
}
