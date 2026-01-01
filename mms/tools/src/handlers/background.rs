use mms_execpolicy::{Evaluation, default_heuristics};
use mms_linux_sandbox::{apply_sandbox_policy, is_sandbox_supported};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Unique identifier for a background process
pub type ProcessId = String;

/// Status of a background process
#[derive(Debug, Clone)]
pub enum ProcessStatus {
    Running,
    Completed { exit_code: i32 },
    Failed { error: String },
    Killed,
}

/// A background process with its output buffer
struct BackgroundProcess {
    #[allow(dead_code)]
    child: Child,
    status: ProcessStatus,
    stdout_buffer: Vec<String>,
    stderr_buffer: Vec<String>,
    started_at: Instant,
}

/// Manager for background processes
pub struct BackgroundProcessManager {
    processes: HashMap<ProcessId, BackgroundProcess>,
    next_id: u64,
}

impl BackgroundProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_id: 1,
        }
    }

    fn generate_id(&mut self) -> ProcessId {
        let id = format!("bg-{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_process(&mut self, child: Child) -> ProcessId {
        let id = self.generate_id();
        let process = BackgroundProcess {
            child,
            status: ProcessStatus::Running,
            stdout_buffer: Vec::new(),
            stderr_buffer: Vec::new(),
            started_at: Instant::now(),
        };
        self.processes.insert(id.clone(), process);
        id
    }

    pub fn get_status(&self, id: &ProcessId) -> Option<ProcessStatus> {
        self.processes.get(id).map(|p| p.status.clone())
    }

    pub fn get_output(&self, id: &ProcessId) -> Option<(Vec<String>, Vec<String>)> {
        self.processes.get(id).map(|p| {
            (p.stdout_buffer.clone(), p.stderr_buffer.clone())
        })
    }

    pub fn list_processes(&self) -> Vec<(ProcessId, ProcessStatus, u64)> {
        self.processes
            .iter()
            .map(|(id, p)| {
                (id.clone(), p.status.clone(), p.started_at.elapsed().as_millis() as u64)
            })
            .collect()
    }

    pub fn update_status(&mut self, id: &ProcessId, status: ProcessStatus) {
        if let Some(process) = self.processes.get_mut(id) {
            process.status = status;
        }
    }

    pub fn append_stdout(&mut self, id: &ProcessId, line: String) {
        if let Some(process) = self.processes.get_mut(id) {
            process.stdout_buffer.push(line);
        }
    }

    pub fn append_stderr(&mut self, id: &ProcessId, line: String) {
        if let Some(process) = self.processes.get_mut(id) {
            process.stderr_buffer.push(line);
        }
    }

    /// Remove and return ownership of the child process for killing
    pub async fn kill_process(&mut self, id: &ProcessId) -> Result<(), String> {
        if let Some(mut process) = self.processes.remove(id) {
            process.child.kill().await.map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err(format!("Process not found: {}", id))
        }
    }

    pub fn has_process(&self, id: &ProcessId) -> bool {
        self.processes.contains_key(id)
    }
}

impl Default for BackgroundProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler for starting background shell commands
pub struct BackgroundShellHandler {
    manager: Arc<Mutex<BackgroundProcessManager>>,
}

impl BackgroundShellHandler {
    pub fn new(manager: Arc<Mutex<BackgroundProcessManager>>) -> Self {
        Self { manager }
    }

    fn check_command(&self, ctx: &ToolContext, command: &[String]) -> Evaluation {
        ctx.exec_policy()
            .map(|p| p.check(command))
            .unwrap_or_else(|| {
                let decision = default_heuristics(command);
                Evaluation { decision, matched_rules: vec![] }
            })
    }

    fn parse_command_to_vec(command: &str) -> Vec<String> {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };
        vec![shell.to_string(), shell_arg.to_string(), command.to_string()]
    }
}

impl ToolHandler for BackgroundShellHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("background_shell", "Execute a shell command in the background")
            .with_parameters(
                json!({
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute in the background"
                    }
                }),
                vec!["command".into()],
            )
    }

    fn execute(&self, ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let command = call.get_string("command").unwrap_or_default();
        let cwd = ctx.cwd().clone();
        let call_id = call.id.clone();
        let sandbox_policy = ctx.sandbox_policy().clone();
        let sandbox_enabled = ctx.sandbox_enabled();
        let manager = self.manager.clone();

        // Check exec policy
        let command_vec = Self::parse_command_to_vec(&command);
        let eval = self.check_command(ctx, &command_vec);

        if eval.is_forbidden() {
            return Box::pin(async move {
                Ok(ToolOutput::error(
                    call_id,
                    format!("Command forbidden by policy: {}", command),
                    0,
                ))
            });
        }

        Box::pin(async move {
            let start = Instant::now();

            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

            // Determine if we should sandbox
            let should_sandbox = sandbox_enabled
                && is_sandbox_supported()
                && !sandbox_policy.has_full_disk_write_access();

            if should_sandbox {
                let sandbox = sandbox_policy.clone().add_writable_root(&cwd);
                if let Err(e) = apply_sandbox_policy(&sandbox, &cwd) {
                    tracing::warn!("Failed to apply sandbox policy: {:?}, continuing without sandbox", e);
                }
            }

            let mut child = Command::new(shell)
                .arg(shell_arg)
                .arg(&command)
                .current_dir(&cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| mms_common::AgentError::Io { source: e })?;

            // Take stdout and stderr for async reading
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // Add process to manager
            let process_id = {
                let mut mgr = manager.lock().await;
                mgr.add_process(child)
            };

            // Spawn tasks to read stdout and stderr
            let manager_stdout = manager.clone();
            let process_id_stdout = process_id.clone();
            if let Some(stdout) = stdout {
                tokio::spawn(async move {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let mut mgr = manager_stdout.lock().await;
                        mgr.append_stdout(&process_id_stdout, line);
                    }
                });
            }

            let manager_stderr = manager.clone();
            let process_id_stderr = process_id.clone();
            if let Some(stderr) = stderr {
                tokio::spawn(async move {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let mut mgr = manager_stderr.lock().await;
                        mgr.append_stderr(&process_id_stderr, line);
                    }
                });
            }

            let duration_ms = start.elapsed().as_millis() as u64;

            let sandbox_status = if should_sandbox { " [sandboxed]" } else { "" };
            let content = format!(
                "Background process started{}\nProcess ID: {}",
                sandbox_status, process_id
            );

            Ok(ToolOutput::success(call_id, content, duration_ms))
        })
    }

    fn is_dangerous(&self, call: &ToolCall) -> bool {
        let command = call.get_string("command").unwrap_or_default();
        let command_vec = Self::parse_command_to_vec(&command);
        let eval_decision = default_heuristics(&command_vec);
        !eval_decision.is_allowed()
    }
}

/// Handler for getting background process status and output
pub struct BackgroundStatusHandler {
    manager: Arc<Mutex<BackgroundProcessManager>>,
}

impl BackgroundStatusHandler {
    pub fn new(manager: Arc<Mutex<BackgroundProcessManager>>) -> Self {
        Self { manager }
    }
}

impl ToolHandler for BackgroundStatusHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("background_status", "Get status and output of a background process")
            .with_parameters(
                json!({
                    "process_id": {
                        "type": "string",
                        "description": "The process ID to check (optional, lists all if not provided)"
                    }
                }),
                vec![],
            )
    }

    fn execute(&self, _ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let process_id = call.get_string("process_id");
        let call_id = call.id.clone();
        let manager = self.manager.clone();

        Box::pin(async move {
            let start = Instant::now();
            let mgr = manager.lock().await;

            let content = if let Some(id) = process_id {
                if let Some(status) = mgr.get_status(&id) {
                    let (stdout, stderr) = mgr.get_output(&id).unwrap_or_default();
                    format!(
                        "Process ID: {}\nStatus: {:?}\n\nStdout ({} lines):\n{}\n\nStderr ({} lines):\n{}",
                        id,
                        status,
                        stdout.len(),
                        stdout.join("\n"),
                        stderr.len(),
                        stderr.join("\n")
                    )
                } else {
                    format!("Process not found: {}", id)
                }
            } else {
                let processes = mgr.list_processes();
                if processes.is_empty() {
                    "No background processes".to_string()
                } else {
                    let lines: Vec<String> = processes
                        .iter()
                        .map(|(id, status, duration)| {
                            format!("{}: {:?} (running for {}ms)", id, status, duration)
                        })
                        .collect();
                    format!("Background processes:\n{}", lines.join("\n"))
                }
            };

            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(ToolOutput::success(call_id, content, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        false
    }
}

/// Handler for killing a background process
pub struct BackgroundKillHandler {
    manager: Arc<Mutex<BackgroundProcessManager>>,
}

impl BackgroundKillHandler {
    pub fn new(manager: Arc<Mutex<BackgroundProcessManager>>) -> Self {
        Self { manager }
    }
}

impl ToolHandler for BackgroundKillHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("background_kill", "Kill a running background process")
            .with_parameters(
                json!({
                    "process_id": {
                        "type": "string",
                        "description": "The process ID to kill"
                    }
                }),
                vec!["process_id".into()],
            )
    }

    fn execute(&self, _ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let process_id = call.get_string("process_id").unwrap_or_default();
        let call_id = call.id.clone();
        let manager = self.manager.clone();

        Box::pin(async move {
            let start = Instant::now();
            let mut mgr = manager.lock().await;

            let content = match mgr.kill_process(&process_id).await {
                Ok(_) => format!("Process {} killed successfully", process_id),
                Err(e) => e,
            };

            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(ToolOutput::success(call_id, content, duration_ms))
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        true // Killing processes is potentially dangerous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_manager_add_and_get() {
        let mut manager = BackgroundProcessManager::new();

        // We can't easily create a Child in tests, but we can test the ID generation
        assert_eq!(manager.generate_id(), "bg-1");
        assert_eq!(manager.generate_id(), "bg-2");
    }

    #[test]
    fn test_process_manager_list_empty() {
        let manager = BackgroundProcessManager::new();
        assert!(manager.list_processes().is_empty());
    }

    #[test]
    fn test_background_shell_handler_spec() {
        let manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
        let handler = BackgroundShellHandler::new(manager);
        let spec = handler.spec();
        assert_eq!(spec.name, "background_shell");
    }

    #[test]
    fn test_background_status_handler_spec() {
        let manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
        let handler = BackgroundStatusHandler::new(manager);
        let spec = handler.spec();
        assert_eq!(spec.name, "background_status");
    }

    #[test]
    fn test_background_kill_handler_spec() {
        let manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
        let handler = BackgroundKillHandler::new(manager);
        let spec = handler.spec();
        assert_eq!(spec.name, "background_kill");
    }
}
