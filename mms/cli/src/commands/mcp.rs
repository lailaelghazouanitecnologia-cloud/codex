use anyhow::Result;
use tracing::info;

pub async fn mcp_list() -> Result<()> {
    info!("Listing MCP servers");

    println!("MCP Servers");
    println!("===========");
    println!();
    println!("No MCP servers configured.");
    println!();
    println!("To add an MCP server:");
    println!("  mms mcp add <name> <command>");
    println!();
    println!("Example:");
    println!("  mms mcp add filesystem \"npx -y @modelcontextprotocol/server-filesystem /path/to/dir\"");
    println!();

    Ok(())
}

pub async fn mcp_add(name: &str, command: &str) -> Result<()> {
    info!("Adding MCP server: {} -> {}", name, command);

    println!("Adding MCP Server");
    println!("=================");
    println!();
    println!("Name: {}", name);
    println!("Command: {}", command);
    println!();
    println!("MCP server configuration will be saved to config.");
    println!("(This feature is under development)");
    println!();

    Ok(())
}

pub async fn mcp_remove(name: &str) -> Result<()> {
    info!("Removing MCP server: {}", name);

    println!("Removing MCP Server");
    println!("===================");
    println!();
    println!("Name: {}", name);
    println!();
    println!("Server removed from configuration.");
    println!("(This feature is under development)");
    println!();

    Ok(())
}
