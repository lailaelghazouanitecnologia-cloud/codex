use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use mms_mcp_types::{
    CallToolResult, GetPromptResult, Implementation, InitializeRequestParams,
    InitializeResult, ListPromptsResult, ListResourcesResult, MCP_SCHEMA_VERSION,
    Prompt, Tool,
};
use tokio::sync::RwLock;

use crate::connection::McpConnection;
use crate::transport::StdioTransport;
use crate::{build_qualified_tool_name, split_qualified_tool_name};

#[derive(Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: OsString,
    pub args: Vec<OsString>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<PathBuf>,
}

struct ServerEntry {
    #[allow(dead_code)]
    config: McpServerConfig,
    connection: McpConnection<StdioTransport>,
    tools: Vec<Tool>,
}

pub struct McpClient {
    servers: RwLock<HashMap<String, Arc<ServerEntry>>>,
    client_info: Implementation,
}

impl McpClient {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
            client_info: Implementation::new(name, version),
        }
    }

    pub async fn add_server(&self, config: McpServerConfig) -> Result<InitializeResult> {
        let transport = StdioTransport::spawn(
            config.command.clone(),
            config.args.clone(),
            config.env.clone(),
            config.cwd.clone(),
        )
        .await?;

        let connection = McpConnection::new(transport);

        let init_params = InitializeRequestParams::new(
            self.client_info.clone(),
            MCP_SCHEMA_VERSION,
        );

        let init_result = connection.initialize(init_params).await?;
        let tools_result = connection.list_tools().await?;

        let entry = Arc::new(ServerEntry {
            config: config.clone(),
            connection,
            tools: tools_result.tools,
        });

        let mut servers = self.servers.write().await;
        servers.insert(config.name.clone(), entry);

        Ok(init_result)
    }

    pub async fn remove_server(&self, name: &str) -> Result<()> {
        let mut servers = self.servers.write().await;

        if servers.remove(name).is_none() {
            return Err(anyhow!("Server '{}' not found", name));
        }

        Ok(())
    }

    pub async fn list_all_tools(&self) -> HashMap<String, Tool> {
        let servers = self.servers.read().await;
        let mut all_tools = HashMap::new();

        for (server_name, entry) in servers.iter() {
            for tool in &entry.tools {
                let qualified_name = build_qualified_tool_name(server_name, &tool.name);
                all_tools.insert(qualified_name, tool.clone());
            }
        }

        all_tools
    }

    pub async fn list_server_tools(&self, server_name: &str) -> Result<Vec<Tool>> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        Ok(entry.tools.clone())
    }

    pub async fn call_tool(
        &self,
        qualified_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let (server_name, tool_name) = split_qualified_tool_name(qualified_name)
            .ok_or_else(|| anyhow!("Invalid qualified tool name: {}", qualified_name))?;

        let servers = self.servers.read().await;

        let entry = servers.get(&server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        entry.connection.call_tool(&tool_name, arguments).await
    }

    pub async fn list_resources(&self, server_name: &str) -> Result<ListResourcesResult> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        entry.connection.list_resources().await
    }

    pub async fn read_resource(&self, server_name: &str, uri: &str) -> Result<mms_mcp_types::ReadResourceResult> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        entry.connection.read_resource(uri).await
    }

    pub async fn list_prompts(&self, server_name: &str) -> Result<ListPromptsResult> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        entry.connection.list_prompts().await
    }

    pub async fn list_all_prompts(&self) -> HashMap<String, Prompt> {
        let servers = self.servers.read().await;
        let mut all_prompts = HashMap::new();

        for (server_name, entry) in servers.iter() {
            if let Ok(result) = entry.connection.list_prompts().await {
                for prompt in result.prompts {
                    let qualified_name = build_qualified_tool_name(server_name, &prompt.name);
                    all_prompts.insert(qualified_name, prompt);
                }
            }
        }

        all_prompts
    }

    pub async fn get_prompt(
        &self,
        server_name: &str,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<GetPromptResult> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        entry.connection.get_prompt(name, arguments).await
    }

    pub async fn ping(&self, server_name: &str) -> Result<()> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        entry.connection.ping().await
    }

    pub async fn ping_all(&self) -> HashMap<String, Result<()>> {
        let servers = self.servers.read().await;
        let mut results = HashMap::new();

        for (name, entry) in servers.iter() {
            let result = entry.connection.ping().await;
            results.insert(name.clone(), result);
        }

        results
    }

    pub async fn server_count(&self) -> usize {
        let servers = self.servers.read().await;
        servers.len()
    }

    pub async fn server_names(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }

    pub async fn has_capability(&self, server_name: &str, capability: &str) -> Result<bool> {
        let servers = self.servers.read().await;

        let entry = servers.get(server_name).ok_or_else(|| {
            anyhow!("Server '{}' not found", server_name)
        })?;

        Ok(entry.connection.has_capability(capability).await)
    }
}
