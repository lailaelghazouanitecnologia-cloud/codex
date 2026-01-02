//! MCP (Model Context Protocol) manager for server configuration and integration.
//!
//! This module handles:
//! - Loading/saving MCP server configurations
//! - Spawning and managing MCP server connections
//! - Providing a tool handler for MCP tools

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use mms_mcp_client::{McpClient, McpServerConfig};
use mms_mcp_types::Tool;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for MCP servers, stored in config file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServersConfig {
    /// Map of server name to server configuration
    #[serde(default)]
    pub servers: HashMap<String, McpServerEntry>,
}

/// Configuration for a single MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    /// Command to run the server (e.g., "npx")
    pub command: String,
    /// Arguments to the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory for the server
    pub cwd: Option<PathBuf>,
    /// Whether to auto-start this server
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl McpServerEntry {
    /// Create a new MCP server entry from a command string
    pub fn from_command(command: &str) -> Self {
        // Parse command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();
        let (cmd, args) = if parts.is_empty() {
            (command.to_string(), vec![])
        } else {
            (
                parts[0].to_string(),
                parts[1..].iter().map(|s| s.to_string()).collect(),
            )
        };

        Self {
            command: cmd,
            args,
            env: HashMap::new(),
            cwd: None,
            enabled: true,
        }
    }

    /// Convert to McpServerConfig for the client
    pub fn to_client_config(&self, name: &str) -> McpServerConfig {
        McpServerConfig {
            name: name.to_string(),
            command: OsString::from(&self.command),
            args: self.args.iter().map(OsString::from).collect(),
            env: if self.env.is_empty() {
                None
            } else {
                Some(self.env.clone())
            },
            cwd: self.cwd.clone(),
        }
    }
}

/// Manager for MCP servers and their tools
pub struct McpManager {
    /// The MCP client
    client: Arc<McpClient>,
    /// Configuration
    config: RwLock<McpServersConfig>,
    /// Path to config file
    config_path: PathBuf,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            client: Arc::new(McpClient::new("mms", env!("CARGO_PKG_VERSION"))),
            config: RwLock::new(McpServersConfig::default()),
            config_path,
        }
    }

    /// Load configuration from file
    pub async fn load_config(&self) -> Result<()> {
        if !self.config_path.exists() {
            debug!("MCP config file does not exist, using defaults");
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.config_path).await?;
        let loaded: McpServersConfig = toml::from_str(&content)?;

        let mut config = self.config.write().await;
        *config = loaded;

        info!("Loaded {} MCP server configurations", config.servers.len());
        Ok(())
    }

    /// Save configuration to file
    pub async fn save_config(&self) -> Result<()> {
        let config = self.config.read().await;
        let content = toml::to_string_pretty(&*config)?;

        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.config_path, content).await?;
        info!("Saved MCP configuration to {}", self.config_path.display());
        Ok(())
    }

    /// Initialize all enabled servers
    pub async fn start_servers(&self) -> Result<()> {
        let config = self.config.read().await;

        for (name, entry) in &config.servers {
            if !entry.enabled {
                debug!("Skipping disabled MCP server: {}", name);
                continue;
            }

            let client_config = entry.to_client_config(name);

            match self.client.add_server(client_config).await {
                Ok(result) => {
                    info!(
                        "Started MCP server '{}': {} v{}",
                        name, result.server_info.name, result.server_info.version
                    );
                }
                Err(e) => {
                    error!("Failed to start MCP server '{}': {}", name, e);
                }
            }
        }

        Ok(())
    }

    /// Add a new server configuration
    pub async fn add_server(&self, name: &str, command: &str) -> Result<()> {
        let entry = McpServerEntry::from_command(command);
        let client_config = entry.to_client_config(name);

        // Try to start the server first to validate the configuration
        let result = self.client.add_server(client_config).await?;

        // If successful, save to config
        {
            let mut config = self.config.write().await;
            config.servers.insert(name.to_string(), entry);
        }

        self.save_config().await?;

        info!(
            "Added MCP server '{}': {} v{}",
            name, result.server_info.name, result.server_info.version
        );

        Ok(())
    }

    /// Remove a server configuration
    pub async fn remove_server(&self, name: &str) -> Result<()> {
        // Remove from client
        if let Err(e) = self.client.remove_server(name).await {
            warn!("Server '{}' was not running: {}", name, e);
        }

        // Remove from config
        {
            let mut config = self.config.write().await;
            if config.servers.remove(name).is_none() {
                return Err(anyhow!("Server '{}' not found in configuration", name));
            }
        }

        self.save_config().await?;

        info!("Removed MCP server '{}'", name);
        Ok(())
    }

    /// List all configured servers
    pub async fn list_servers(&self) -> Vec<(String, bool)> {
        let config = self.config.read().await;
        config
            .servers
            .iter()
            .map(|(name, entry)| (name.clone(), entry.enabled))
            .collect()
    }

    /// Get all available MCP tools
    pub async fn list_tools(&self) -> HashMap<String, Tool> {
        self.client.list_all_tools().await
    }

    /// Get tools for a specific server
    pub async fn list_server_tools(&self, server_name: &str) -> Result<Vec<Tool>> {
        self.client.list_server_tools(server_name).await
    }

    /// Call an MCP tool
    pub async fn call_tool(
        &self,
        qualified_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<String> {
        let result = self.client.call_tool(qualified_name, arguments).await?;

        // Convert result content to string
        let content_str = result
            .content
            .iter()
            .map(|c| c.to_text())
            .collect::<Vec<_>>()
            .join("\n");

        if result.is_error.unwrap_or(false) {
            Err(anyhow!("MCP tool error: {}", content_str))
        } else {
            Ok(content_str)
        }
    }

    /// Check if a tool name is an MCP tool
    pub fn is_mcp_tool(name: &str) -> bool {
        name.starts_with("mcp__")
    }

    /// Get the MCP client
    pub fn client(&self) -> &Arc<McpClient> {
        &self.client
    }

    /// Ping all servers
    pub async fn ping_all(&self) -> HashMap<String, bool> {
        let results = self.client.ping_all().await;
        results
            .into_iter()
            .map(|(name, result)| (name, result.is_ok()))
            .collect()
    }

    /// Get server count
    pub async fn server_count(&self) -> usize {
        self.client.server_count().await
    }
}

/// Extension trait for MCP content blocks
trait McpContentExt {
    fn to_text(&self) -> String;
}

impl McpContentExt for mms_mcp_types::ContentBlock {
    fn to_text(&self) -> String {
        match self {
            mms_mcp_types::ContentBlock::Text(t) => t.text.clone(),
            mms_mcp_types::ContentBlock::Image(i) => {
                format!("[Image: {} ({} bytes)]", i.mime_type, i.data.len())
            }
            mms_mcp_types::ContentBlock::Audio(a) => {
                format!("[Audio: {} ({} bytes)]", a.mime_type, a.data.len())
            }
            mms_mcp_types::ContentBlock::Resource(r) => {
                format!("[Resource: {}]", r.uri)
            }
            mms_mcp_types::ContentBlock::Embedded(e) => {
                format!("[Embedded: {:?}]", e.r#type)
            }
        }
    }
}
