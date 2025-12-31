use agent_common::{AgentError, AgentResult};
use std::sync::Arc;
use std::time::Instant;

use crate::context::ToolContext;
use crate::registry::ToolRegistry;
use crate::spec::{ToolCall, ToolOutput};

pub struct ToolRouter {
    registry: Arc<ToolRegistry>,
}

impl ToolRouter {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    pub async fn execute(&self, ctx: &ToolContext, call: ToolCall) -> AgentResult<ToolOutput> {
        let start = Instant::now();
        let call_id = call.id.clone();

        let handler = self.registry.get(&call.name).ok_or_else(|| {
            AgentError::not_found(format!("tool not found: {}", call.name))
        })?;

        let result = handler.execute(ctx, call).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut output) => {
                output.duration_ms = duration_ms;
                Ok(output)
            }
            Err(e) => Ok(ToolOutput::error(call_id, e.to_string(), duration_ms)),
        }
    }

    pub fn is_dangerous(&self, call: &ToolCall) -> bool {
        self.registry
            .get(&call.name)
            .map(|h| h.is_dangerous(call))
            .unwrap_or(true)
    }
}
