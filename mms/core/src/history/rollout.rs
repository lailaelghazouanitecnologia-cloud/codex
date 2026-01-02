//! Rollout recording for session persistence.
//!
//! This module provides JSONL-based persistence of conversation history,
//! allowing sessions to be resumed or reviewed later.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::session::ConversationId;

/// Errors that can occur during rollout operations.
#[derive(Debug, Error)]
pub enum RolloutError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Channel closed.
    #[error("Recorder channel closed")]
    ChannelClosed,
}

/// Result type for rollout operations.
pub type RolloutResult<T> = Result<T, RolloutError>;

/// Session metadata for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    /// Conversation ID.
    pub id: ConversationId,

    /// ISO 8601 timestamp.
    pub timestamp: String,

    /// Working directory.
    pub cwd: PathBuf,

    /// Session originator (e.g., "cli", "vscode").
    pub originator: String,

    /// CLI/app version.
    pub version: String,

    /// Custom instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Model provider ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,

    /// Model ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// A line in the rollout JSONL file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutLine {
    /// ISO 8601 timestamp with milliseconds.
    pub timestamp: String,

    /// The item being recorded.
    #[serde(flatten)]
    pub item: RolloutItem,
}

impl RolloutLine {
    /// Create a new rollout line with the current timestamp.
    pub fn new(item: RolloutItem) -> Self {
        Self {
            timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            item,
        }
    }
}

/// Types of items that can be recorded in the rollout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum RolloutItem {
    /// Session metadata.
    SessionMeta(SessionMeta),

    /// User message.
    UserMessage(UserMessageItem),

    /// Assistant message.
    AssistantMessage(AssistantMessageItem),

    /// Tool call.
    ToolCall(ToolCallItem),

    /// Tool result.
    ToolResult(ToolResultItem),

    /// Reasoning content.
    Reasoning(ReasoningItem),

    /// Compaction summary.
    Compacted(CompactedItem),

    /// Turn context snapshot.
    TurnContext(TurnContextItem),

    /// Generic event.
    Event(serde_json::Value),
}

/// User message item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageItem {
    /// Message content.
    pub content: String,

    /// Optional images (base64 encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

/// Assistant message item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageItem {
    /// Message content.
    pub content: String,
}

/// Tool call item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallItem {
    /// Tool call ID.
    pub id: String,

    /// Tool name.
    pub name: String,

    /// Tool arguments.
    pub arguments: serde_json::Value,
}

/// Tool result item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultItem {
    /// Tool call ID this result corresponds to.
    pub call_id: String,

    /// Result content.
    pub content: serde_json::Value,

    /// Whether the tool call succeeded.
    #[serde(default = "default_true")]
    pub success: bool,
}

fn default_true() -> bool {
    true
}

/// Reasoning content item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningItem {
    /// Reasoning content.
    pub content: String,
}

/// Compaction summary item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactedItem {
    /// Summary text.
    pub summary: String,

    /// Number of items compacted.
    pub items_compacted: usize,
}

/// Turn context snapshot item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnContextItem {
    /// Working directory.
    pub cwd: PathBuf,

    /// Model being used.
    pub model: String,

    /// Approval policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,

    /// Sandbox policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<String>,
}

/// Command for the rollout writer task.
enum RolloutCmd {
    /// Write items to file.
    Write(Vec<RolloutLine>),

    /// Flush the buffer.
    Flush,

    /// Shutdown the writer.
    Shutdown,
}

/// Rollout recorder for persisting conversation history.
#[derive(Debug)]
pub struct RolloutRecorder {
    /// Channel sender for commands.
    tx: mpsc::Sender<RolloutCmd>,

    /// Path to the rollout file.
    pub rollout_path: PathBuf,
}

impl RolloutRecorder {
    /// Create a new rollout recorder.
    ///
    /// Creates the rollout file and spawns a background writer task.
    pub async fn new(
        base_dir: &Path,
        conversation_id: &ConversationId,
        meta: SessionMeta,
    ) -> RolloutResult<Self> {
        // Create directory structure: base_dir/sessions/YYYY/MM/DD/
        let now = Utc::now();
        let session_dir = base_dir
            .join("sessions")
            .join(now.format("%Y").to_string())
            .join(now.format("%m").to_string())
            .join(now.format("%d").to_string());

        tokio::fs::create_dir_all(&session_dir).await?;

        // Create filename: rollout-YYYY-MM-DDThh-mm-ss-{id}.jsonl
        let filename = format!(
            "rollout-{}-{}.jsonl",
            now.format("%Y-%m-%dT%H-%M-%S"),
            conversation_id
        );
        let rollout_path = session_dir.join(filename);

        // Create file and writer
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&rollout_path)
            .await?;

        let writer = BufWriter::new(file);

        // Create channel
        let (tx, rx) = mpsc::channel::<RolloutCmd>(256);

        // Spawn writer task
        tokio::spawn(writer_task(rx, writer));

        let recorder = Self {
            tx,
            rollout_path,
        };

        // Write session metadata as first line
        recorder.record_item(RolloutItem::SessionMeta(meta)).await?;

        Ok(recorder)
    }

    /// Record a single item.
    pub async fn record_item(&self, item: RolloutItem) -> RolloutResult<()> {
        let line = RolloutLine::new(item);
        self.record_lines(vec![line]).await
    }

    /// Record multiple items.
    pub async fn record_items(&self, items: Vec<RolloutItem>) -> RolloutResult<()> {
        let lines: Vec<RolloutLine> = items.into_iter().map(RolloutLine::new).collect();
        self.record_lines(lines).await
    }

    /// Record rollout lines.
    async fn record_lines(&self, lines: Vec<RolloutLine>) -> RolloutResult<()> {
        self.tx
            .send(RolloutCmd::Write(lines))
            .await
            .map_err(|_| RolloutError::ChannelClosed)
    }

    /// Flush the writer buffer.
    pub async fn flush(&self) -> RolloutResult<()> {
        self.tx
            .send(RolloutCmd::Flush)
            .await
            .map_err(|_| RolloutError::ChannelClosed)
    }

    /// Shutdown the recorder.
    pub async fn shutdown(&self) -> RolloutResult<()> {
        let _ = self.tx.send(RolloutCmd::Shutdown).await;
        Ok(())
    }

    /// Get rollout path.
    pub fn path(&self) -> &Path {
        &self.rollout_path
    }
}

/// Background writer task.
async fn writer_task(
    mut rx: mpsc::Receiver<RolloutCmd>,
    mut writer: BufWriter<File>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::Write(lines) => {
                for line in lines {
                    match serde_json::to_string(&line) {
                        Ok(json) => {
                            if let Err(e) = writer.write_all(json.as_bytes()).await {
                                error!("Failed to write rollout line: {}", e);
                            }
                            if let Err(e) = writer.write_all(b"\n").await {
                                error!("Failed to write newline: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize rollout line: {}", e);
                        }
                    }
                }
            }
            RolloutCmd::Flush => {
                if let Err(e) = writer.flush().await {
                    error!("Failed to flush rollout: {}", e);
                }
            }
            RolloutCmd::Shutdown => {
                if let Err(e) = writer.flush().await {
                    error!("Failed to flush rollout on shutdown: {}", e);
                }
                debug!("Rollout writer shutdown");
                break;
            }
        }
    }
}

/// Load rollout history from a file.
pub async fn load_rollout_history(path: &Path) -> RolloutResult<Vec<RolloutLine>> {
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines_reader = reader.lines();

    let mut items = Vec::new();

    while let Some(line) = lines_reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<RolloutLine>(&line) {
            Ok(item) => items.push(item),
            Err(e) => {
                warn!("Failed to parse rollout line: {} - {}", e, line);
            }
        }
    }

    Ok(items)
}

/// Get session metadata from rollout history.
pub fn get_session_meta(history: &[RolloutLine]) -> Option<&SessionMeta> {
    history.iter().find_map(|line| {
        if let RolloutItem::SessionMeta(meta) = &line.item {
            Some(meta)
        } else {
            None
        }
    })
}

/// List available rollout files in a directory.
pub async fn list_rollout_files(base_dir: &Path) -> RolloutResult<Vec<PathBuf>> {
    let sessions_dir = base_dir.join("sessions");

    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_rollout_files(&sessions_dir, &mut files).await?;

    // Sort by modification time (newest first)
    files.sort_by(|a, b| {
        let a_time = std::fs::metadata(a).and_then(|m| m.modified()).ok();
        let b_time = std::fs::metadata(b).and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    Ok(files)
}

/// Recursively collect rollout files.
async fn collect_rollout_files(dir: &Path, files: &mut Vec<PathBuf>) -> RolloutResult<()> {
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            Box::pin(collect_rollout_files(&path, files)).await?;
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                files.push(path);
            }
        }
    }

    Ok(())
}

/// Shared rollout recorder.
pub type SharedRolloutRecorder = Arc<RolloutRecorder>;
