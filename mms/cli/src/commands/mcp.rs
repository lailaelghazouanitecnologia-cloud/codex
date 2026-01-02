//! MCP (Model Context Protocol) CLI commands

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use mms_tools::McpManager;
use tracing::info;

/// Get the path to the MCP servers config file
fn get_mcp_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent").join("mcp-servers.toml"))
        .unwrap_or_else(|| PathBuf::from(".agent/mcp-servers.toml"))
}

/// Create and initialize an MCP manager
async fn create_manager() -> Result<Arc<McpManager>> {
    let config_path = get_mcp_config_path();
    let manager = Arc::new(McpManager::new(config_path));
    manager.load_config().await?;
    Ok(manager)
}

/// List all configured MCP servers
pub async fn mcp_list() -> Result<()> {
    info!("Listing MCP servers");

    let manager = create_manager().await?;
    let servers = manager.list_servers().await;

    println!("MCP Servers");
    println!("===========");
    println!();

    if servers.is_empty() {
        println!("No MCP servers configured.");
        println!();
        println!("To add an MCP server:");
        println!("  mms mcp add <name> <command>");
        println!();
        println!("Example:");
        println!("  mms mcp add filesystem \"npx -y @modelcontextprotocol/server-filesystem /path/to/dir\"");
        println!();
    } else {
        for (name, enabled) in servers {
            let status = if enabled { "enabled" } else { "disabled" };
            println!("  {} ({})", name, status);
        }
        println!();
        println!("Total: {} server(s)", manager.list_servers().await.len());
        println!();

        // Try to start servers and list tools
        if let Err(e) = manager.start_servers().await {
            eprintln!("Warning: Failed to start some servers: {}", e);
        }

        let tools = manager.list_tools().await;
        if !tools.is_empty() {
            println!("Available MCP Tools:");
            for (name, tool) in &tools {
                let desc = tool.description.as_deref().unwrap_or("");
                println!("  {} - {}", name, desc);
            }
            println!();
        }
    }

    Ok(())
}

/// Add a new MCP server
pub async fn mcp_add(name: &str, command: &str) -> Result<()> {
    info!("Adding MCP server: {} -> {}", name, command);

    let manager = create_manager().await?;

    println!("Adding MCP Server");
    println!("=================");
    println!();
    println!("Name: {}", name);
    println!("Command: {}", command);
    println!();

    match manager.add_server(name, command).await {
        Ok(()) => {
            println!("Server added successfully!");
            println!();

            // List available tools from the server
            let tools = manager.list_server_tools(name).await?;
            if !tools.is_empty() {
                println!("Available tools:");
                for tool in &tools {
                    let desc = tool.description.as_deref().unwrap_or("");
                    println!("  mcp__{}__{}  - {}", name, tool.name, desc);
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("Failed to add server: {}", e);
            eprintln!();
            eprintln!("Make sure the command is valid and the server can be started.");
            return Err(e);
        }
    }

    Ok(())
}

/// Remove an MCP server
pub async fn mcp_remove(name: &str) -> Result<()> {
    info!("Removing MCP server: {}", name);

    let manager = create_manager().await?;

    println!("Removing MCP Server");
    println!("===================");
    println!();
    println!("Name: {}", name);
    println!();

    match manager.remove_server(name).await {
        Ok(()) => {
            println!("Server removed successfully!");
        }
        Err(e) => {
            eprintln!("Failed to remove server: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
