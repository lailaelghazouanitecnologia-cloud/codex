//! MCP tool handler for routing tool calls to MCP servers.

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tokio::sync::RwLock;

use crate::context::ToolContext;
use crate::handler::{ToolFuture, ToolHandler};
use crate::mcp::McpManager;
use crate::spec::{ToolCall, ToolOutput, ToolSpec};

/// Handler for MCP tools that routes calls to MCP servers.
///
/// This handler is dynamically registered for each MCP tool discovered
/// from connected MCP servers.
pub struct McpToolHandler {
    /// The MCP manager
    manager: Arc<McpManager>,
    /// Tool specification
    spec: ToolSpec,
    /// Qualified tool name (mcp__server__tool)
    qualified_name: String,
    /// Whether this tool is dangerous
    dangerous: bool,
}

impl McpToolHandler {
    /// Create a new MCP tool handler
    pub fn new(
        manager: Arc<McpManager>,
        qualified_name: String,
        spec: ToolSpec,
        dangerous: bool,
    ) -> Self {
        Self {
            manager,
            spec,
            qualified_name,
            dangerous,
        }
    }
}

impl ToolHandler for McpToolHandler {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn execute(&self, _ctx: &ToolContext, call: ToolCall) -> ToolFuture {
        let manager = self.manager.clone();
        let qualified_name = self.qualified_name.clone();
        let call_id = call.id.clone();
        let arguments = if call.arguments.is_null() {
            None
        } else {
            Some(call.arguments.clone())
        };

        Box::pin(async move {
            let start = Instant::now();

            match manager.call_tool(&qualified_name, arguments).await {
                Ok(content) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    Ok(ToolOutput::success(call_id, content, duration_ms))
                }
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    Ok(ToolOutput::error(call_id, e.to_string(), duration_ms))
                }
            }
        })
    }

    fn is_dangerous(&self, _call: &ToolCall) -> bool {
        self.dangerous
    }
}

/// Registry for MCP tools that can be discovered dynamically
pub struct McpToolRegistry {
    manager: Arc<McpManager>,
    handlers: RwLock<Vec<Arc<McpToolHandler>>>,
}

impl McpToolRegistry {
    /// Create a new MCP tool registry
    pub fn new(manager: Arc<McpManager>) -> Self {
        Self {
            manager,
            handlers: RwLock::new(Vec::new()),
        }
    }

    /// Refresh the tool registry from MCP servers
    pub async fn refresh(&self) {
        let tools = self.manager.list_tools().await;
        let mut handlers = self.handlers.write().await;
        handlers.clear();

        for (qualified_name, tool) in tools {
            // Convert MCP tool to ToolSpec
            let spec = ToolSpec::new(&qualified_name, &tool.description.unwrap_or_default())
                .with_parameters(
                    tool.input_schema.properties.unwrap_or_else(|| json!({})),
                    tool.input_schema.required.unwrap_or_default(),
                );

            // Determine if tool is dangerous based on name patterns
            let dangerous = is_dangerous_tool(&tool.name);

            let handler = Arc::new(McpToolHandler::new(
                self.manager.clone(),
                qualified_name,
                spec,
                dangerous,
            ));

            handlers.push(handler);
        }
    }

    /// Get all tool specs
    pub async fn specs(&self) -> Vec<ToolSpec> {
        let handlers = self.handlers.read().await;
        handlers.iter().map(|h| h.spec()).collect()
    }

    /// Find a handler by name
    pub async fn find(&self, name: &str) -> Option<Arc<McpToolHandler>> {
        let handlers = self.handlers.read().await;
        handlers.iter().find(|h| h.spec.name == name).cloned()
    }

    /// Get the manager
    pub fn manager(&self) -> &Arc<McpManager> {
        &self.manager
    }
}

/// Determine if an MCP tool is dangerous based on its name
fn is_dangerous_tool(name: &str) -> bool {
    let dangerous_patterns = [
        "write",
        "delete",
        "remove",
        "execute",
        "run",
        "shell",
        "command",
        "create",
        "modify",
        "update",
        "set",
        "put",
        "post",
        "patch",
    ];

    let name_lower = name.to_lowercase();
    dangerous_patterns
        .iter()
        .any(|pattern| name_lower.contains(pattern))
}
