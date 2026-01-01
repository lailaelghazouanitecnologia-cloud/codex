use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Result;
use mms_mcp_types::JsonRpcMessage;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub trait Transport: Send + Sync {
    fn send(&self, message: JsonRpcMessage) -> impl std::future::Future<Output = Result<()>> + Send;
    fn receive(&self) -> impl std::future::Future<Output = Result<Option<JsonRpcMessage>>> + Send;
}

pub struct StdioTransport {
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    #[allow(dead_code)]
    child: Mutex<Child>,
}

impl StdioTransport {
    pub async fn spawn(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<String, String>>,
        cwd: Option<PathBuf>,
    ) -> io::Result<Self> {
        let mut command = Command::new(program);

        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(&args);

        if let Some(envs) = env {
            command.envs(envs);
        }

        if let Some(dir) = cwd {
            command.current_dir(dir);
        }

        let mut child = command.spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "Failed to capture stdin")
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other, "Failed to capture stdout")
        })?;

        if let Some(stderr) = child.stderr.take() {
            let program_name = args.first().map_or("unknown".to_string(), |a| {
                a.to_string_lossy().to_string()
            });
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                loop {
                    match reader.next_line().await {
                        Ok(Some(line)) => {
                            debug!(target: "mcp", "Server stderr ({program_name}): {line}");
                        }
                        Ok(None) => break,
                        Err(error) => {
                            warn!(target: "mcp", "Error reading stderr ({program_name}): {error}");
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self {
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            child: Mutex::new(child),
        })
    }
}

impl Transport for StdioTransport {
    async fn send(&self, message: JsonRpcMessage) -> Result<()> {
        let json = serde_json::to_string(&message)?;
        let mut stdin = self.stdin.lock().await;

        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        Ok(())
    }

    async fn receive(&self) -> Result<Option<JsonRpcMessage>> {
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();

        let bytes_read = stdout.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None);
        }

        let message: JsonRpcMessage = serde_json::from_str(line.trim())?;
        Ok(Some(message))
    }
}
