//! MCP Connection Manager for managing multiple MCP server connections.
//!
//! This module provides a high-level manager for MCP server lifecycle,
//! including async initialization, tool aggregation, and health monitoring.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use mms_mcp_types::{CallToolResult, Tool};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::connection::McpConnection;
use crate::transport::StdioTransport;
use crate::{build_qualified_tool_name, split_qualified_tool_name};

/// Default timeout for server startup.
pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Default timeout for tool calls.
pub const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(60);

/// Status of an MCP server during startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpStartupStatus {
    /// Server is starting.
    Starting,
    /// Server is ready.
    Ready { tool_count: usize },
    /// Server startup failed.
    Failed { error: String },
    /// Server was cancelled.
    Cancelled,
}

/// Event emitted during MCP startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStartupEvent {
    /// Server name.
    pub server_name: String,
    /// Status of the server.
    pub status: McpStartupStatus,
}

/// Complete event after all servers have been processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStartupCompleteEvent {
    /// Total number of servers.
    pub total: usize,
    /// Number of successful servers.
    pub successful: usize,
    /// Number of failed servers.
    pub failed: usize,
}

/// Tool filter for controlling which tools are exposed.
#[derive(Debug, Clone, Default)]
pub struct ToolFilter {
    /// If Some, only these tools are enabled.
    enabled: Option<HashSet<String>>,
    /// These tools are always disabled.
    disabled: HashSet<String>,
}

impl ToolFilter {
    /// Create a new tool filter.
    pub fn new(enabled: Option<Vec<String>>, disabled: Option<Vec<String>>) -> Self {
        Self {
            enabled: enabled.map(|v| v.into_iter().collect()),
            disabled: disabled.unwrap_or_default().into_iter().collect(),
        }
    }

    /// Check if a tool is allowed by this filter.
    pub fn allows(&self, tool_name: &str) -> bool {
        // Check denylist first
        if self.disabled.contains(tool_name) {
            return false;
        }

        // Check allowlist if present
        match &self.enabled {
            Some(enabled) => enabled.contains(tool_name),
            None => true,
        }
    }
}

/// Configuration for an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfiguration {
    /// Server name.
    pub name: String,

    /// Command to run.
    pub command: String,

    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    /// Working directory.
    #[serde(default)]
    pub cwd: Option<PathBuf>,

    /// Whether the server is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Startup timeout.
    #[serde(default)]
    pub startup_timeout: Option<Duration>,

    /// Tool call timeout.
    #[serde(default)]
    pub tool_timeout: Option<Duration>,

    /// List of enabled tools (allowlist).
    #[serde(default)]
    pub enabled_tools: Option<Vec<String>>,

    /// List of disabled tools (denylist).
    #[serde(default)]
    pub disabled_tools: Option<Vec<String>>,
}

fn default_enabled() -> bool {
    true
}

/// Tool info with server context.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Server name.
    pub server_name: String,
    /// Original tool name.
    pub tool_name: String,
    /// Tool definition.
    pub tool: Tool,
}

/// Managed client entry.
struct ManagedClient {
    connection: McpConnection<StdioTransport>,
    tools: Vec<ToolInfo>,
    tool_filter: ToolFilter,
    tool_timeout: Duration,
}

/// Error during server startup.
#[derive(Debug, Clone)]
pub enum StartupError {
    /// Startup was cancelled.
    Cancelled,
    /// Startup failed.
    Failed(String),
    /// Startup timed out.
    Timeout,
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartupError::Cancelled => write!(f, "startup cancelled"),
            StartupError::Failed(msg) => write!(f, "{}", msg),
            StartupError::Timeout => write!(f, "startup timed out"),
        }
    }
}

/// MCP Connection Manager for managing multiple server connections.
pub struct McpConnectionManager {
    clients: RwLock<HashMap<String, Arc<ManagedClient>>>,
    cancel_token: CancellationToken,
}

impl Default for McpConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpConnectionManager {
    /// Create a new connection manager.
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            cancel_token: CancellationToken::new(),
        }
    }

    /// Create a new connection manager with a cancellation token.
    pub fn with_cancel_token(cancel_token: CancellationToken) -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            cancel_token,
        }
    }

    /// Initialize multiple servers.
    ///
    /// Returns a receiver for startup events.
    pub async fn initialize<F>(
        &self,
        servers: HashMap<String, McpServerConfiguration>,
        mut on_event: F,
    ) -> McpStartupCompleteEvent
    where
        F: FnMut(McpStartupEvent) + Send + 'static,
    {
        let total = servers.len();
        let mut successful = 0;
        let mut failed = 0;

        // Filter enabled servers
        let enabled_servers: Vec<_> = servers
            .into_iter()
            .filter(|(_, cfg)| cfg.enabled)
            .collect();

        // Start all servers in parallel
        let mut handles = Vec::new();

        for (name, config) in enabled_servers {
            let cancel_token = self.cancel_token.clone();

            on_event(McpStartupEvent {
                server_name: name.clone(),
                status: McpStartupStatus::Starting,
            });

            let handle = tokio::spawn(async move {
                let result = start_server_task(&config, cancel_token).await;
                (name, config, result)
            });

            handles.push(handle);
        }

        // Collect results
        for handle in handles {
            match handle.await {
                Ok((name, config, result)) => {
                    match result {
                        Ok(client) => {
                            let tool_count = client.tools.len();

                            // Add to clients
                            let mut clients = self.clients.write().await;
                            clients.insert(name.clone(), Arc::new(client));

                            on_event(McpStartupEvent {
                                server_name: name,
                                status: McpStartupStatus::Ready { tool_count },
                            });

                            successful += 1;
                        }
                        Err(e) => {
                            let error_msg = match e {
                                StartupError::Cancelled => "cancelled".to_string(),
                                StartupError::Timeout => format!(
                                    "timed out after {:?} (adjust startup_timeout to increase)",
                                    config.startup_timeout.unwrap_or(DEFAULT_STARTUP_TIMEOUT)
                                ),
                                StartupError::Failed(msg) => msg,
                            };

                            on_event(McpStartupEvent {
                                server_name: name,
                                status: McpStartupStatus::Failed { error: error_msg },
                            });

                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    error!("Server task panicked: {}", e);
                    failed += 1;
                }
            }
        }

        McpStartupCompleteEvent {
            total,
            successful,
            failed,
        }
    }

    /// Add a single server.
    pub async fn add_server(&self, config: McpServerConfiguration) -> Result<usize> {
        if !config.enabled {
            return Ok(0);
        }

        let client = start_server_task(&config, self.cancel_token.clone())
            .await
            .map_err(|e| anyhow!("Failed to start server {}: {}", config.name, e))?;

        let tool_count = client.tools.len();

        let mut clients = self.clients.write().await;
        clients.insert(config.name.clone(), Arc::new(client));

        Ok(tool_count)
    }

    /// Remove a server.
    pub async fn remove_server(&self, name: &str) -> bool {
        let mut clients = self.clients.write().await;
        clients.remove(name).is_some()
    }

    /// List all tools from all servers.
    pub async fn list_all_tools(&self) -> HashMap<String, ToolInfo> {
        let clients = self.clients.read().await;
        let mut all_tools = HashMap::new();

        for (_, client) in clients.iter() {
            for tool_info in &client.tools {
                if client.tool_filter.allows(&tool_info.tool_name) {
                    let qualified = build_qualified_tool_name(
                        &tool_info.server_name,
                        &tool_info.tool_name,
                    );
                    all_tools.insert(qualified, tool_info.clone());
                }
            }
        }

        all_tools
    }

    /// Call a tool by qualified name.
    pub async fn call_tool(
        &self,
        qualified_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let (server_name, tool_name) = split_qualified_tool_name(qualified_name)
            .ok_or_else(|| anyhow!("Invalid qualified tool name: {}", qualified_name))?;

        let clients = self.clients.read().await;
        let client = clients.get(&server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        // Check tool filter
        if !client.tool_filter.allows(&tool_name) {
            return Err(anyhow!("Tool '{}' is not allowed by filter", tool_name));
        }

        // Call with timeout
        let timeout = client.tool_timeout;
        let result = tokio::time::timeout(
            timeout,
            client.connection.call_tool(&tool_name, arguments),
        )
        .await
        .map_err(|_| anyhow!("Tool call timed out after {:?}", timeout))?;

        result
    }

    /// Get server names.
    pub async fn server_names(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    /// Get number of connected servers.
    pub async fn server_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

    /// Check if a server is connected.
    pub async fn has_server(&self, name: &str) -> bool {
        let clients = self.clients.read().await;
        clients.contains_key(name)
    }

    /// Ping a server.
    pub async fn ping(&self, server_name: &str) -> Result<()> {
        let clients = self.clients.read().await;
        let client = clients.get(server_name)
            .ok_or_else(|| anyhow!("Server '{}' not found", server_name))?;

        client.connection.ping().await
    }

    /// Ping all servers.
    pub async fn ping_all(&self) -> HashMap<String, Result<()>> {
        let clients = self.clients.read().await;
        let mut results = HashMap::new();

        for (name, client) in clients.iter() {
            let result = client.connection.ping().await;
            results.insert(name.clone(), result);
        }

        results
    }

    /// Cancel all pending operations.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Shutdown all servers.
    pub async fn shutdown(&self) {
        self.cancel_token.cancel();
        let mut clients = self.clients.write().await;
        clients.clear();
        info!("MCP connection manager shutdown complete");
    }
}

/// Start a server task.
async fn start_server_task(
    config: &McpServerConfiguration,
    cancel_token: CancellationToken,
) -> Result<ManagedClient, StartupError> {
    let startup_timeout = config.startup_timeout.unwrap_or(DEFAULT_STARTUP_TIMEOUT);
    let tool_timeout = config.tool_timeout.unwrap_or(DEFAULT_TOOL_TIMEOUT);

    // Create tool filter
    let tool_filter = ToolFilter::new(
        config.enabled_tools.clone(),
        config.disabled_tools.clone(),
    );

    // Run with timeout and cancellation
    let startup = async {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Err(StartupError::Cancelled);
        }

        // Spawn the transport
        debug!("Starting MCP server: {} ({})", config.name, config.command);

        let command = OsString::from(&config.command);
        let args: Vec<OsString> = config.args.iter().map(OsString::from).collect();

        let transport = StdioTransport::spawn(
            command,
            args,
            config.env.clone(),
            config.cwd.clone(),
        )
        .await
        .map_err(|e| StartupError::Failed(format!("Failed to spawn: {}", e)))?;

        let connection = McpConnection::new(transport);

        // Initialize the connection
        let init_params = mms_mcp_types::InitializeRequestParams::new(
            mms_mcp_types::Implementation::new("mms", env!("CARGO_PKG_VERSION")),
            mms_mcp_types::MCP_SCHEMA_VERSION,
        );

        connection
            .initialize(init_params)
            .await
            .map_err(|e| StartupError::Failed(format!("Initialize failed: {}", e)))?;

        // Send initialized notification
        connection
            .send_notification("notifications/initialized", serde_json::json!({}))
            .await
            .map_err(|e| StartupError::Failed(format!("Initialized notification failed: {}", e)))?;

        // List tools
        let tools_result = connection
            .list_tools()
            .await
            .map_err(|e| StartupError::Failed(format!("List tools failed: {}", e)))?;

        // Convert to ToolInfo
        let tools: Vec<ToolInfo> = tools_result
            .tools
            .into_iter()
            .map(|tool| ToolInfo {
                server_name: config.name.clone(),
                tool_name: tool.name.clone(),
                tool,
            })
            .collect();

        info!(
            "MCP server {} ready with {} tools",
            config.name,
            tools.len()
        );

        Ok(ManagedClient {
            connection,
            tools,
            tool_filter,
            tool_timeout,
        })
    };

    // Apply timeout
    tokio::select! {
        result = startup => result,
        _ = tokio::time::sleep(startup_timeout) => {
            warn!("MCP server {} startup timed out", config.name);
            Err(StartupError::Timeout)
        }
        _ = cancel_token.cancelled() => {
            info!("MCP server {} startup cancelled", config.name);
            Err(StartupError::Cancelled)
        }
    }
}

/// Type alias for shared connection manager.
pub type SharedMcpConnectionManager = Arc<RwLock<McpConnectionManager>>;

/// Create a new shared connection manager.
pub fn new_mcp_connection_manager() -> SharedMcpConnectionManager {
    Arc::new(RwLock::new(McpConnectionManager::new()))
}
