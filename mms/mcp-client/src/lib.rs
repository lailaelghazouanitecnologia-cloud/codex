mod client;
mod connection;
mod transport;

pub use client::{McpClient, McpServerConfig};
pub use connection::McpConnection;
pub use transport::{StdioTransport, Transport};

pub const MCP_TOOL_NAME_PREFIX: &str = "mcp";
pub const MCP_TOOL_NAME_DELIMITER: &str = "__";

pub fn build_qualified_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "{}{}{}{}{}",
        MCP_TOOL_NAME_PREFIX,
        MCP_TOOL_NAME_DELIMITER,
        server_name,
        MCP_TOOL_NAME_DELIMITER,
        tool_name
    )
}

pub fn split_qualified_tool_name(qualified_name: &str) -> Option<(String, String)> {
    let mut parts = qualified_name.split(MCP_TOOL_NAME_DELIMITER);

    let prefix = parts.next()?;
    if prefix != MCP_TOOL_NAME_PREFIX {
        return None;
    }

    let server_name = parts.next()?;
    let tool_name: String = parts.collect::<Vec<_>>().join(MCP_TOOL_NAME_DELIMITER);

    if tool_name.is_empty() {
        return None;
    }

    Some((server_name.to_string(), tool_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_qualified_name_works() {
        let result = build_qualified_tool_name("server1", "tool1");
        assert_eq!(result, "mcp__server1__tool1");
    }

    #[test]
    fn split_qualified_name_works() {
        let result = split_qualified_tool_name("mcp__server1__tool1");
        assert_eq!(result, Some(("server1".to_string(), "tool1".to_string())));
    }

    #[test]
    fn split_qualified_name_handles_nested_names() {
        let result = split_qualified_tool_name("mcp__alpha__nested__op");
        assert_eq!(result, Some(("alpha".to_string(), "nested__op".to_string())));
    }

    #[test]
    fn split_qualified_name_rejects_invalid() {
        assert_eq!(split_qualified_tool_name("other__server__tool"), None);
        assert_eq!(split_qualified_tool_name("mcp__server__"), None);
    }
}
