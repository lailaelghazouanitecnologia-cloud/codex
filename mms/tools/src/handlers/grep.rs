//! Grep handler for searching content in files.

use regex::Regex;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Handler for searching file contents using regex patterns.
pub struct GrepHandler;

/// Maximum matches to return per file
const MAX_MATCHES_PER_FILE: usize = 20;

/// Maximum total matches to return
const MAX_TOTAL_MATCHES: usize = 100;

/// Context lines before and after match
const DEFAULT_CONTEXT_LINES: usize = 0;

impl ToolHandler for GrepHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "grep",
            "Search for a pattern in files using regex. Returns matching lines with context.",
        )
        .with_parameters(
            json!({
                "pattern": {
                    "type": "string",
                    "description": "The regex pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "The directory or file to search in. Defaults to current directory.",
                    "default": "."
                },
                "glob": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., '*.rs', '**/*.ts')",
                    "default": "**/*"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Perform case-insensitive matching. Default is false.",
                    "default": false
                },
                "context": {
                    "type": "integer",
                    "description": "Number of context lines to show before and after each match. Default is 0.",
                    "default": 0
                },
                "files_only": {
                    "type": "boolean",
                    "description": "Only show file names with matches, not the matching content. Default is false.",
                    "default": false
                }
            }),
            vec!["pattern".into()],
        )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let pattern = call.get_string("pattern").unwrap_or_default();
        let path = call.get_string("path").unwrap_or_else(|| ".".to_string());
        let glob_pattern = call.get_string("glob").unwrap_or_else(|| "**/*".to_string());
        let case_insensitive = call.get_bool("case_insensitive").unwrap_or(false);
        let context_lines = call.get_i64("context").unwrap_or(DEFAULT_CONTEXT_LINES as i64) as usize;
        let files_only = call.get_bool("files_only").unwrap_or(false);
        let resolved_path = ctx.resolve_path(&path);
        let call_id = call.id.clone();

        Box::pin(async move {
            let start = Instant::now();

            // Build regex
            let regex_pattern = if case_insensitive {
                format!("(?i){}", pattern)
            } else {
                pattern.clone()
            };

            let regex = match Regex::new(&regex_pattern) {
                Ok(r) => r,
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(ToolOutput::error(
                        call_id,
                        format!("Invalid regex pattern: {}", e),
                        duration_ms,
                    ));
                }
            };

            // Find files to search
            let files = find_files(&resolved_path, &glob_pattern);

            if files.is_empty() {
                let duration_ms = start.elapsed().as_millis() as u64;
                return Ok(ToolOutput::success(
                    call_id,
                    format!("No files found matching glob pattern: {}", glob_pattern),
                    duration_ms,
                ));
            }

            // Search files
            let mut results: Vec<FileMatch> = Vec::new();
            let mut total_matches = 0;

            for file_path in files {
                if total_matches >= MAX_TOTAL_MATCHES {
                    break;
                }

                // Skip binary files
                if is_likely_binary(&file_path) {
                    continue;
                }

                // Read file content
                let content = match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(_) => continue, // Skip files we can't read
                };

                let lines: Vec<&str> = content.lines().collect();
                let mut file_matches = Vec::new();

                for (line_num, line) in lines.iter().enumerate() {
                    if regex.is_match(line) {
                        let match_info = MatchInfo {
                            line_number: line_num + 1,
                            content: line.to_string(),
                            context_before: get_context(&lines, line_num, context_lines, true),
                            context_after: get_context(&lines, line_num, context_lines, false),
                        };
                        file_matches.push(match_info);
                        total_matches += 1;

                        if file_matches.len() >= MAX_MATCHES_PER_FILE {
                            break;
                        }
                        if total_matches >= MAX_TOTAL_MATCHES {
                            break;
                        }
                    }
                }

                if !file_matches.is_empty() {
                    let relative_path = file_path
                        .strip_prefix(&resolved_path)
                        .unwrap_or(&file_path)
                        .to_path_buf();

                    results.push(FileMatch {
                        path: relative_path,
                        matches: file_matches,
                    });
                }
            }

            let duration_ms = start.elapsed().as_millis() as u64;

            if results.is_empty() {
                return Ok(ToolOutput::success(
                    call_id,
                    format!("No matches found for pattern: {}", pattern),
                    duration_ms,
                ));
            }

            // Format output
            let output = format_results(&results, files_only, total_matches, context_lines > 0);

            Ok(ToolOutput::success(call_id, output, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        false // Searching files is safe
    }
}

#[derive(Debug)]
struct FileMatch {
    path: PathBuf,
    matches: Vec<MatchInfo>,
}

#[derive(Debug)]
struct MatchInfo {
    line_number: usize,
    content: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

fn find_files(base_path: &std::path::Path, glob_pattern: &str) -> Vec<PathBuf> {
    let full_pattern = base_path.join(glob_pattern);
    let pattern_str = full_pattern.to_string_lossy().to_string();

    match glob::glob(&pattern_str) {
        Ok(paths) => paths
            .filter_map(|p| p.ok())
            .filter(|p| p.is_file())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn is_likely_binary(path: &PathBuf) -> bool {
    // Check by extension
    let binary_extensions = [
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg",
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
        "zip", "tar", "gz", "bz2", "7z", "rar",
        "exe", "dll", "so", "dylib", "bin",
        "wasm", "pyc", "class",
        "mp3", "mp4", "avi", "mkv", "wav",
        "ttf", "otf", "woff", "woff2",
        "db", "sqlite",
    ];

    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if binary_extensions.contains(&ext_lower.as_str()) {
            return true;
        }
    }

    // Check first few bytes for null characters
    if let Ok(mut file) = std::fs::File::open(path) {
        use std::io::Read;
        let mut buffer = [0u8; 1024];
        if let Ok(n) = file.read(&mut buffer) {
            return buffer[..n].contains(&0);
        }
    }

    false
}

fn get_context(lines: &[&str], current_line: usize, context_size: usize, before: bool) -> Vec<String> {
    if context_size == 0 {
        return Vec::new();
    }

    if before {
        let start = current_line.saturating_sub(context_size);
        (start..current_line)
            .map(|i| lines[i].to_string())
            .collect()
    } else {
        let end = (current_line + context_size + 1).min(lines.len());
        ((current_line + 1)..end)
            .map(|i| lines[i].to_string())
            .collect()
    }
}

fn format_results(results: &[FileMatch], files_only: bool, total_matches: usize, has_context: bool) -> String {
    let mut output = String::new();

    if files_only {
        output.push_str(&format!("Found {} file(s) with matches:\n\n", results.len()));
        for file_match in results {
            output.push_str(&format!("{} ({} matches)\n", file_match.path.display(), file_match.matches.len()));
        }
    } else {
        output.push_str(&format!("Found {} matches in {} file(s):\n\n", total_matches, results.len()));

        for file_match in results {
            output.push_str(&format!("=== {} ===\n", file_match.path.display()));

            for m in &file_match.matches {
                // Context before
                for (i, ctx_line) in m.context_before.iter().enumerate() {
                    let ctx_line_num = m.line_number - (m.context_before.len() - i);
                    output.push_str(&format!("  {}: {}\n", ctx_line_num, ctx_line));
                }

                // The match line
                output.push_str(&format!("> {}: {}\n", m.line_number, m.content));

                // Context after
                for (i, ctx_line) in m.context_after.iter().enumerate() {
                    output.push_str(&format!("  {}: {}\n", m.line_number + i + 1, ctx_line));
                }

                if has_context {
                    output.push_str("---\n");
                }
            }
            output.push('\n');
        }
    }

    if total_matches >= MAX_TOTAL_MATCHES {
        output.push_str(&format!("(limited to {} total matches)\n", MAX_TOTAL_MATCHES));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_context(temp_dir: &TempDir) -> ToolContext {
        let config = mms_config::Config::default();
        let mut ctx = ToolContext::new(Arc::new(config));
        ctx.set_cwd(temp_dir.path().to_path_buf());
        ctx
    }

    #[tokio::test]
    async fn test_grep_basic() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file.txt"), "Hello World\nGoodbye World").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GrepHandler;

        let call = ToolCall::new(
            "test-1",
            "grep",
            json!({ "pattern": "Hello" }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("Hello World"));
        assert!(!result.content.contains("Goodbye"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file.txt"), "HELLO World\nhello world").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GrepHandler;

        let call = ToolCall::new(
            "test-1",
            "grep",
            json!({ "pattern": "hello", "case_insensitive": true }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("HELLO"));
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_grep_files_only() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "match here").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "no match here either").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GrepHandler;

        let call = ToolCall::new(
            "test-1",
            "grep",
            json!({ "pattern": "match", "files_only": true }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("file1.txt"));
        assert!(result.content.contains("file2.txt"));
        assert!(!result.content.contains("match here")); // Content not shown
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file.txt"), "Hello World").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GrepHandler;

        let call = ToolCall::new(
            "test-1",
            "grep",
            json!({ "pattern": "NotFound" }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_grep_with_glob() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file.rs"), "fn main() {}").unwrap();
        std::fs::write(temp_dir.path().join("file.txt"), "fn main() {}").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GrepHandler;

        let call = ToolCall::new(
            "test-1",
            "grep",
            json!({ "pattern": "fn main", "glob": "*.rs" }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("file.rs"));
        assert!(!result.content.contains("file.txt"));
    }
}
