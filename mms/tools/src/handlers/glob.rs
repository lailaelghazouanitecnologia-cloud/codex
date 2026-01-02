//! Glob handler for finding files by pattern.

use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Handler for finding files using glob patterns.
pub struct GlobHandler;

impl ToolHandler for GlobHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "glob",
            "Find files matching a glob pattern (e.g., '**/*.rs', 'src/**/*.ts').",
        )
        .with_parameters(
            json!({
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (e.g., '**/*.rs', 'src/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "The base directory to search in. Defaults to current directory.",
                    "default": "."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of files to return. Default is 100.",
                    "default": 100
                }
            }),
            vec!["pattern".into()],
        )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let pattern = call.get_string("pattern").unwrap_or_default();
        let path = call.get_string("path").unwrap_or_else(|| ".".to_string());
        let limit = call.get_i64("limit").unwrap_or(100) as usize;
        let resolved_path = ctx.resolve_path(&path);
        let call_id = call.id.clone();

        Box::pin(async move {
            let start = Instant::now();

            // Build the full pattern
            let full_pattern = if pattern.starts_with('/') || pattern.starts_with("**") {
                resolved_path.join(&pattern)
            } else {
                resolved_path.join(&pattern)
            };

            let pattern_str = full_pattern.to_string_lossy().to_string();

            // Use glob to find matching files
            let matches: Vec<PathBuf> = match glob::glob(&pattern_str) {
                Ok(paths) => paths
                    .filter_map(|p| p.ok())
                    .take(limit)
                    .collect(),
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return Ok(ToolOutput::error(
                        call_id,
                        format!("Invalid glob pattern: {}", e),
                        duration_ms,
                    ));
                }
            };

            let duration_ms = start.elapsed().as_millis() as u64;

            if matches.is_empty() {
                return Ok(ToolOutput::success(
                    call_id,
                    format!("No files found matching pattern: {}", pattern),
                    duration_ms,
                ));
            }

            // Format output
            let mut output = format!("Found {} file(s) matching '{}':\n\n", matches.len(), pattern);

            for path in &matches {
                // Try to show relative path from base
                let display_path = path
                    .strip_prefix(&resolved_path)
                    .unwrap_or(path);
                output.push_str(&format!("{}\n", display_path.display()));
            }

            if matches.len() >= limit {
                output.push_str(&format!("\n(limited to {} results)", limit));
            }

            Ok(ToolOutput::success(call_id, output, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        false // Reading file paths is safe
    }
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
    async fn test_glob_basic() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();
        std::fs::write(temp_dir.path().join("file.rs"), "content").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GlobHandler;

        let call = ToolCall::new(
            "test-1",
            "glob",
            json!({ "pattern": "*.txt" }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("file1.txt"));
        assert!(result.content.contains("file2.txt"));
        assert!(!result.content.contains("file.rs"));
    }

    #[tokio::test]
    async fn test_glob_recursive() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("root.rs"), "content").unwrap();
        std::fs::write(temp_dir.path().join("subdir/nested.rs"), "content").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GlobHandler;

        let call = ToolCall::new(
            "test-1",
            "glob",
            json!({ "pattern": "**/*.rs" }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("root.rs"));
        assert!(result.content.contains("nested.rs"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file.txt"), "content").unwrap();

        let ctx = create_test_context(&temp_dir);
        let handler = GlobHandler;

        let call = ToolCall::new(
            "test-1",
            "glob",
            json!({ "pattern": "*.rs" }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("No files found"));
    }

    #[tokio::test]
    async fn test_glob_limit() {
        let temp_dir = TempDir::new().unwrap();
        for i in 0..10 {
            std::fs::write(temp_dir.path().join(format!("file{}.txt", i)), "content").unwrap();
        }

        let ctx = create_test_context(&temp_dir);
        let handler = GlobHandler;

        let call = ToolCall::new(
            "test-1",
            "glob",
            json!({ "pattern": "*.txt", "limit": 3 }),
        );

        let result = handler.execute(&ctx, call).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("limited to 3 results"));
    }
}
