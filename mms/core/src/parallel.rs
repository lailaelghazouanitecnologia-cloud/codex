//! Parallel tool execution with concurrency control.
//!
//! Provides utilities for running multiple tool calls concurrently
//! with configurable limits and cancellation support.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

use crate::cancel::CancellationToken;

/// Default maximum concurrent tool executions
pub const DEFAULT_MAX_CONCURRENT: usize = 5;

/// Configuration for parallel execution
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Maximum concurrent executions
    pub max_concurrent: usize,
    /// Whether to stop all on first error
    pub fail_fast: bool,
    /// Whether to respect tool ordering (false = truly parallel)
    pub preserve_order: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            fail_fast: false,
            preserve_order: false,
        }
    }
}

impl ParallelConfig {
    /// Create with specified concurrency limit
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent,
            ..Default::default()
        }
    }

    /// Enable fail-fast mode
    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Enable order preservation
    pub fn with_preserve_order(mut self, preserve_order: bool) -> Self {
        self.preserve_order = preserve_order;
        self
    }
}

/// Result of a single tool execution
#[derive(Debug)]
pub struct ToolExecutionResult<T> {
    /// The tool call ID
    pub call_id: String,
    /// The result (None if cancelled/failed to start)
    pub result: Option<T>,
    /// Execution order (index in original list)
    pub index: usize,
    /// Whether this was cancelled
    pub cancelled: bool,
}

/// Executor for parallel tool calls
pub struct ParallelExecutor {
    config: ParallelConfig,
    semaphore: Arc<Semaphore>,
}

impl ParallelExecutor {
    /// Create a new parallel executor
    pub fn new(config: ParallelConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
        Self { config, semaphore }
    }

    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(ParallelConfig::default())
    }

    /// Execute multiple tool calls in parallel
    ///
    /// The executor function receives (call_id, index) and returns the result.
    pub async fn execute<T, F, Fut>(
        &self,
        call_ids: Vec<String>,
        executor: F,
    ) -> Vec<ToolExecutionResult<T>>
    where
        T: Send + 'static,
        F: Fn(String, usize) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        let cancel_token = CancellationToken::new();
        self.execute_with_cancellation(call_ids, executor, &cancel_token)
            .await
    }

    /// Execute with cancellation support
    pub async fn execute_with_cancellation<T, F, Fut>(
        &self,
        call_ids: Vec<String>,
        executor: F,
        cancel_token: &CancellationToken,
    ) -> Vec<ToolExecutionResult<T>>
    where
        T: Send + 'static,
        F: Fn(String, usize) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
    {
        if call_ids.is_empty() {
            return vec![];
        }

        let num_calls = call_ids.len();
        debug!(count = num_calls, "Starting parallel tool execution");

        // Create tasks
        let mut handles = Vec::with_capacity(num_calls);
        let fail_fast_token = if self.config.fail_fast {
            Some(CancellationToken::new())
        } else {
            None
        };

        for (index, call_id) in call_ids.into_iter().enumerate() {
            let semaphore = self.semaphore.clone();
            let executor = executor.clone();
            let cancel = cancel_token.clone();
            let fail_fast = fail_fast_token.clone();

            let handle = tokio::spawn(async move {
                // Acquire semaphore permit
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        warn!(call_id = %call_id, "Semaphore closed");
                        return ToolExecutionResult {
                            call_id,
                            result: None,
                            index,
                            cancelled: true,
                        };
                    }
                };

                // Check cancellation
                if cancel.is_cancelled() {
                    debug!(call_id = %call_id, "Execution cancelled");
                    return ToolExecutionResult {
                        call_id,
                        result: None,
                        index,
                        cancelled: true,
                    };
                }

                // Check fail-fast
                if let Some(ref ff) = fail_fast {
                    if ff.is_cancelled() {
                        debug!(call_id = %call_id, "Fail-fast triggered");
                        return ToolExecutionResult {
                            call_id,
                            result: None,
                            index,
                            cancelled: true,
                        };
                    }
                }

                // Execute
                let result = executor(call_id.clone(), index).await;

                ToolExecutionResult {
                    call_id,
                    result: Some(result),
                    index,
                    cancelled: false,
                }
            });

            handles.push(handle);
        }

        // Collect results
        let mut results = Vec::with_capacity(num_calls);
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!(error = %e, "Task panicked");
                }
            }
        }

        // Sort by index if preserving order
        if self.config.preserve_order {
            results.sort_by_key(|r| r.index);
        }

        debug!(
            completed = results.len(),
            total = num_calls,
            "Parallel execution completed"
        );

        results
    }
}

/// Execute tool calls in parallel with a simple interface
///
/// Returns results in the same order as input call_ids.
pub async fn execute_parallel<T, E, F, Fut>(
    call_ids: Vec<String>,
    max_concurrent: usize,
    executor: F,
) -> Vec<Result<T, E>>
where
    T: Send + 'static,
    E: Send + 'static + Default,
    F: Fn(String, usize) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = Result<T, E>> + Send + 'static,
{
    let config = ParallelConfig::new(max_concurrent).with_preserve_order(true);
    let parallel = ParallelExecutor::new(config);
    let results = parallel.execute(call_ids, executor).await;

    // Convert to simple Result vec
    results
        .into_iter()
        .map(|r| r.result.unwrap_or_else(|| Err(E::default())))
        .collect()
}

/// Batch tools by dependency group
///
/// Returns groups that can be executed in parallel within each group,
/// but groups must be executed sequentially.
pub fn batch_by_dependency<T>(
    items: Vec<T>,
    depends_on: impl Fn(&T, &T) -> bool,
) -> Vec<Vec<T>> {
    if items.is_empty() {
        return vec![];
    }

    let mut batches: Vec<Vec<T>> = vec![];

    for item in items {
        // Find the first batch where this item has no dependencies
        let mut target_batch = None;

        for (idx, batch) in batches.iter().enumerate() {
            let has_dependency = batch.iter().any(|b| depends_on(&item, b));
            if !has_dependency {
                target_batch = Some(idx);
                break;
            }
        }

        match target_batch {
            Some(idx) => batches[idx].push(item),
            None => batches.push(vec![item]),
        }
    }

    batches
}

/// Tool call with dependency tracking
#[derive(Debug, Clone)]
pub struct DependentToolCall {
    /// Call ID
    pub call_id: String,
    /// Tool name
    pub tool_name: String,
    /// IDs of calls this depends on
    pub depends_on: Vec<String>,
}

impl DependentToolCall {
    /// Create a new dependent tool call
    pub fn new(call_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            depends_on: vec![],
        }
    }

    /// Add a dependency
    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.depends_on.push(dep.into());
        self
    }

    /// Add multiple dependencies
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.depends_on.extend(deps);
        self
    }
}

/// Scheduler for dependent tool calls
pub struct ToolScheduler {
    calls: Vec<DependentToolCall>,
    completed: HashMap<String, bool>,
}

impl ToolScheduler {
    /// Create a new scheduler
    pub fn new(calls: Vec<DependentToolCall>) -> Self {
        let completed = HashMap::new();
        Self { calls, completed }
    }

    /// Get the next batch of calls that can be executed
    pub fn next_batch(&self) -> Vec<&DependentToolCall> {
        self.calls
            .iter()
            .filter(|call| {
                // Not already completed
                !self.completed.contains_key(&call.call_id) &&
                // All dependencies satisfied
                call.depends_on.iter().all(|dep| {
                    self.completed.get(dep).copied().unwrap_or(false)
                })
            })
            .collect()
    }

    /// Mark a call as completed
    pub fn complete(&mut self, call_id: &str, success: bool) {
        self.completed.insert(call_id.to_string(), success);
    }

    /// Check if all calls are completed
    pub fn is_complete(&self) -> bool {
        self.calls.iter().all(|c| self.completed.contains_key(&c.call_id))
    }

    /// Get remaining (uncompleted) calls
    pub fn remaining(&self) -> Vec<&DependentToolCall> {
        self.calls
            .iter()
            .filter(|c| !self.completed.contains_key(&c.call_id))
            .collect()
    }
}
