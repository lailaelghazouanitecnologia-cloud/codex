use mms_common::{AgentError, AgentResult};
use std::sync::Arc;
use std::time::Instant;

use crate::context::ToolContext;
use crate::registry::ToolRegistry;
use crate::spec::{ToolCall, ToolOutput};

#[derive(Debug, Clone)]
pub struct ParallelConfig {
    pub max_concurrent: usize,
    pub timeout_ms: u64,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            timeout_ms: 60000,
        }
    }
}

pub struct ToolRouter {
    registry: Arc<ToolRegistry>,
    parallel_config: ParallelConfig,
}

impl ToolRouter {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            parallel_config: ParallelConfig::default(),
        }
    }

    pub fn with_parallel_config(mut self, config: ParallelConfig) -> Self {
        self.parallel_config = config;
        self
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

    pub async fn execute_parallel(
        &self,
        ctx: &ToolContext,
        calls: Vec<ToolCall>,
    ) -> Vec<AgentResult<ToolOutput>> {
        if calls.is_empty() {
            return Vec::new();
        }

        if calls.len() == 1 {
            return vec![self.execute(ctx, calls.into_iter().next().unwrap()).await];
        }

        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.parallel_config.max_concurrent));
        let timeout = std::time::Duration::from_millis(self.parallel_config.timeout_ms);

        let futures: Vec<_> = calls
            .into_iter()
            .map(|call| {
                let registry = self.registry.clone();
                let ctx = ctx.clone();
                let sem = semaphore.clone();

                async move {
                    let _permit = sem.acquire().await;
                    let start = Instant::now();
                    let call_id = call.id.clone();

                    let handler = match registry.get(&call.name) {
                        Some(h) => h,
                        None => {
                            return Ok(ToolOutput::error(
                                call_id,
                                format!("tool not found: {}", call.name),
                                0,
                            ))
                        }
                    };

                    let result = tokio::time::timeout(timeout, handler.execute(&ctx, call)).await;
                    let duration_ms = start.elapsed().as_millis() as u64;

                    match result {
                        Ok(Ok(mut output)) => {
                            output.duration_ms = duration_ms;
                            Ok(output)
                        }
                        Ok(Err(e)) => Ok(ToolOutput::error(call_id, e.to_string(), duration_ms)),
                        Err(_) => Ok(ToolOutput::error(
                            call_id,
                            "tool execution timed out".to_string(),
                            duration_ms,
                        )),
                    }
                }
            })
            .collect();

        futures::future::join_all(futures).await
    }

    pub async fn execute_sequential(
        &self,
        ctx: &ToolContext,
        calls: Vec<ToolCall>,
    ) -> Vec<AgentResult<ToolOutput>> {
        let mut results = Vec::with_capacity(calls.len());

        for call in calls {
            results.push(self.execute(ctx, call).await);
        }

        results
    }

    pub fn is_dangerous(&self, call: &ToolCall) -> bool {
        self.registry
            .get(&call.name)
            .map(|h| h.is_dangerous(call))
            .unwrap_or(true)
    }

    pub fn any_dangerous(&self, calls: &[ToolCall]) -> bool {
        calls.iter().any(|c| self.is_dangerous(c))
    }

    pub fn filter_dangerous<'a>(&self, calls: &'a [ToolCall]) -> (Vec<&'a ToolCall>, Vec<&'a ToolCall>) {
        let mut safe = Vec::new();
        let mut dangerous = Vec::new();

        for call in calls {
            if self.is_dangerous(call) {
                dangerous.push(call);
            } else {
                safe.push(call);
            }
        }

        (safe, dangerous)
    }
}
