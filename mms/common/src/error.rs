use thiserror::Error;

/// Primary error type for the agent.
#[derive(Debug, Error)]
pub enum AgentError {
    // ─── Configuration & IO ─────────────────────────────────────────────────

    #[error("configuration error: {message}")]
    Config { message: String },

    #[error("io error: {message}")]
    IoError { message: String },

    #[error("io error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("serialization error: {message}")]
    Serialization { message: String },

    #[error("parse error: {message}")]
    Parse { message: String },

    // ─── Network & Provider ─────────────────────────────────────────────────

    #[error("network error: {message}")]
    Network { message: String },

    #[error("provider error: {message}")]
    Provider { message: String },

    #[error("provider not available: {provider}")]
    ProviderNotAvailable { provider: String },

    #[error("rate limited by {provider}: retry after {retry_after_secs}s")]
    RateLimited { provider: String, retry_after_secs: u64 },

    #[error("quota exceeded for {provider}: {message}")]
    QuotaExceeded { provider: String, message: String },

    #[error("model not found: {model}")]
    ModelNotFound { model: String },

    #[error("context length exceeded: {tokens} tokens (max: {max_tokens})")]
    ContextLengthExceeded { tokens: u64, max_tokens: u64 },

    // ─── Authentication ─────────────────────────────────────────────────────

    #[error("authentication failed: {message}")]
    Authentication { message: String },

    #[error("authentication required for {provider}")]
    AuthRequired { provider: String },

    #[error("token expired for {provider}")]
    TokenExpired { provider: String },

    #[error("invalid api key for {provider}")]
    InvalidApiKey { provider: String },

    #[error("permission denied: {message}")]
    PermissionDenied { message: String },

    // ─── Execution & Tools ──────────────────────────────────────────────────

    #[error("execution error: {message}")]
    Execution { message: String },

    #[error("tool execution failed: {tool_name} - {message}")]
    ToolExecution { tool_name: String, message: String },

    #[error("tool not found: {tool_name}")]
    ToolNotFound { tool_name: String },

    #[error("tool blocked by policy: {tool_name}")]
    ToolBlocked { tool_name: String },

    #[error("tool approval required: {tool_name}")]
    ToolApprovalRequired { tool_name: String },

    #[error("tool approval denied: {tool_name}")]
    ToolApprovalDenied { tool_name: String },

    #[error("sandbox error: {message}")]
    Sandbox { message: String },

    #[error("command blocked: {command}")]
    CommandBlocked { command: String },

    // ─── Session & State ────────────────────────────────────────────────────

    #[error("timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("channel closed unexpectedly")]
    ChannelClosed,

    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("session error: {message}")]
    Session { message: String },

    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },

    #[error("conversation not found: {conversation_id}")]
    ConversationNotFound { conversation_id: String },

    #[error("operation cancelled")]
    Cancelled,

    // ─── MCP (Model Context Protocol) ───────────────────────────────────────

    #[error("MCP connection failed: {server} - {message}")]
    McpConnection { server: String, message: String },

    #[error("MCP server not found: {server}")]
    McpServerNotFound { server: String },

    #[error("MCP server timeout: {server}")]
    McpTimeout { server: String },

    #[error("MCP protocol error: {message}")]
    McpProtocol { message: String },

    // ─── Policy ─────────────────────────────────────────────────────────────

    #[error("policy violation: {message}")]
    PolicyViolation { message: String },

    #[error("policy not found: {policy_name}")]
    PolicyNotFound { policy_name: String },

    #[error("approval timeout: waited {duration_ms}ms")]
    ApprovalTimeout { duration_ms: u64 },

    #[error("approval denied by user")]
    ApprovalDenied,

    // ─── Git ────────────────────────────────────────────────────────────────

    #[error("git error: {message}")]
    Git { message: String },

    #[error("not in a git repository")]
    NotInGitRepo,

    #[error("git operation failed: {operation} - {message}")]
    GitOperation { operation: String, message: String },

    // ─── Context & History ──────────────────────────────────────────────────

    #[error("context error: {message}")]
    Context { message: String },

    #[error("history error: {message}")]
    History { message: String },

    #[error("rollout error: {message}")]
    Rollout { message: String },

    // ─── Resources ──────────────────────────────────────────────────────────

    #[error("resource not found: {resource}")]
    NotFound { resource: String },

    #[error("resource already exists: {resource}")]
    AlreadyExists { resource: String },

    #[error("resource busy: {resource}")]
    ResourceBusy { resource: String },

    // ─── Internal ───────────────────────────────────────────────────────────

    #[error("internal error: {message}")]
    Internal { message: String },

    #[error("not implemented: {feature}")]
    NotImplemented { feature: String },

    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("hook error: {hook_name} - {message}")]
    Hook { hook_name: String, message: String },
}

impl AgentError {
    // ─── Configuration & IO ─────────────────────────────────────────────────

    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    pub fn io_error(message: impl Into<String>) -> Self {
        Self::IoError {
            message: message.into(),
        }
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    // ─── Network & Provider ─────────────────────────────────────────────────

    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider {
            message: message.into(),
        }
    }

    pub fn provider_not_available(provider: impl Into<String>) -> Self {
        Self::ProviderNotAvailable {
            provider: provider.into(),
        }
    }

    pub fn rate_limited(provider: impl Into<String>, retry_after_secs: u64) -> Self {
        Self::RateLimited {
            provider: provider.into(),
            retry_after_secs,
        }
    }

    pub fn quota_exceeded(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::QuotaExceeded {
            provider: provider.into(),
            message: message.into(),
        }
    }

    pub fn model_not_found(model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            model: model.into(),
        }
    }

    pub fn context_length_exceeded(tokens: u64, max_tokens: u64) -> Self {
        Self::ContextLengthExceeded { tokens, max_tokens }
    }

    // ─── Authentication ─────────────────────────────────────────────────────

    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    pub fn auth_required(provider: impl Into<String>) -> Self {
        Self::AuthRequired {
            provider: provider.into(),
        }
    }

    pub fn token_expired(provider: impl Into<String>) -> Self {
        Self::TokenExpired {
            provider: provider.into(),
        }
    }

    pub fn invalid_api_key(provider: impl Into<String>) -> Self {
        Self::InvalidApiKey {
            provider: provider.into(),
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
        }
    }

    // ─── Execution & Tools ──────────────────────────────────────────────────

    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            message: message.into(),
        }
    }

    pub fn tool_execution(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolExecution {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    pub fn tool_not_found(tool_name: impl Into<String>) -> Self {
        Self::ToolNotFound {
            tool_name: tool_name.into(),
        }
    }

    pub fn tool_blocked(tool_name: impl Into<String>) -> Self {
        Self::ToolBlocked {
            tool_name: tool_name.into(),
        }
    }

    pub fn tool_approval_required(tool_name: impl Into<String>) -> Self {
        Self::ToolApprovalRequired {
            tool_name: tool_name.into(),
        }
    }

    pub fn tool_approval_denied(tool_name: impl Into<String>) -> Self {
        Self::ToolApprovalDenied {
            tool_name: tool_name.into(),
        }
    }

    pub fn sandbox(message: impl Into<String>) -> Self {
        Self::Sandbox {
            message: message.into(),
        }
    }

    pub fn command_blocked(command: impl Into<String>) -> Self {
        Self::CommandBlocked {
            command: command.into(),
        }
    }

    // ─── Session & State ────────────────────────────────────────────────────

    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    pub fn invalid_state(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::InvalidStateTransition {
            from: from.into(),
            to: to.into(),
        }
    }

    pub fn session(message: impl Into<String>) -> Self {
        Self::Session {
            message: message.into(),
        }
    }

    pub fn session_not_found(session_id: impl Into<String>) -> Self {
        Self::SessionNotFound {
            session_id: session_id.into(),
        }
    }

    pub fn conversation_not_found(conversation_id: impl Into<String>) -> Self {
        Self::ConversationNotFound {
            conversation_id: conversation_id.into(),
        }
    }

    // ─── MCP ────────────────────────────────────────────────────────────────

    pub fn mcp_connection(server: impl Into<String>, message: impl Into<String>) -> Self {
        Self::McpConnection {
            server: server.into(),
            message: message.into(),
        }
    }

    pub fn mcp_server_not_found(server: impl Into<String>) -> Self {
        Self::McpServerNotFound {
            server: server.into(),
        }
    }

    pub fn mcp_timeout(server: impl Into<String>) -> Self {
        Self::McpTimeout {
            server: server.into(),
        }
    }

    pub fn mcp_protocol(message: impl Into<String>) -> Self {
        Self::McpProtocol {
            message: message.into(),
        }
    }

    // ─── Policy ─────────────────────────────────────────────────────────────

    pub fn policy_violation(message: impl Into<String>) -> Self {
        Self::PolicyViolation {
            message: message.into(),
        }
    }

    pub fn policy_not_found(policy_name: impl Into<String>) -> Self {
        Self::PolicyNotFound {
            policy_name: policy_name.into(),
        }
    }

    pub fn approval_timeout(duration_ms: u64) -> Self {
        Self::ApprovalTimeout { duration_ms }
    }

    // ─── Git ────────────────────────────────────────────────────────────────

    pub fn git(message: impl Into<String>) -> Self {
        Self::Git {
            message: message.into(),
        }
    }

    pub fn git_operation(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self::GitOperation {
            operation: operation.into(),
            message: message.into(),
        }
    }

    // ─── Context & History ──────────────────────────────────────────────────

    pub fn context(message: impl Into<String>) -> Self {
        Self::Context {
            message: message.into(),
        }
    }

    pub fn history(message: impl Into<String>) -> Self {
        Self::History {
            message: message.into(),
        }
    }

    pub fn rollout(message: impl Into<String>) -> Self {
        Self::Rollout {
            message: message.into(),
        }
    }

    // ─── Resources ──────────────────────────────────────────────────────────

    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    pub fn already_exists(resource: impl Into<String>) -> Self {
        Self::AlreadyExists {
            resource: resource.into(),
        }
    }

    pub fn resource_busy(resource: impl Into<String>) -> Self {
        Self::ResourceBusy {
            resource: resource.into(),
        }
    }

    // ─── Internal ───────────────────────────────────────────────────────────

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::NotImplemented {
            feature: feature.into(),
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument {
            message: message.into(),
        }
    }

    pub fn hook(hook_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Hook {
            hook_name: hook_name.into(),
            message: message.into(),
        }
    }

    // ─── Error Classification ───────────────────────────────────────────────

    /// Check if this error is recoverable (can be retried).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. }
                | Self::RateLimited { .. }
                | Self::Network { .. }
                | Self::McpTimeout { .. }
                | Self::ApprovalTimeout { .. }
                | Self::Cancelled
        )
    }

    /// Check if this error is due to authentication issues.
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            Self::Authentication { .. }
                | Self::AuthRequired { .. }
                | Self::TokenExpired { .. }
                | Self::InvalidApiKey { .. }
                | Self::PermissionDenied { .. }
        )
    }

    /// Check if this error is rate-limit related.
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, Self::RateLimited { .. } | Self::QuotaExceeded { .. })
    }

    /// Check if this error requires user approval.
    pub fn requires_approval(&self) -> bool {
        matches!(
            self,
            Self::ToolApprovalRequired { .. } | Self::ApprovalDenied
        )
    }

    /// Check if this error is a policy violation.
    pub fn is_policy_error(&self) -> bool {
        matches!(
            self,
            Self::PolicyViolation { .. }
                | Self::PolicyNotFound { .. }
                | Self::ToolBlocked { .. }
                | Self::CommandBlocked { .. }
        )
    }

    /// Check if this error is an MCP-related error.
    pub fn is_mcp_error(&self) -> bool {
        matches!(
            self,
            Self::McpConnection { .. }
                | Self::McpServerNotFound { .. }
                | Self::McpTimeout { .. }
                | Self::McpProtocol { .. }
        )
    }

    /// Get the retry delay in seconds if applicable.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            Self::RateLimited {
                retry_after_secs, ..
            } => Some(*retry_after_secs),
            Self::Timeout { duration_ms } => Some(duration_ms / 1000),
            _ => None,
        }
    }

    /// Get the error category for logging/metrics.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Config { .. } | Self::IoError { .. } | Self::Io { .. } => "config",
            Self::Serialization { .. } | Self::Parse { .. } => "parse",
            Self::Network { .. } | Self::Provider { .. } | Self::ProviderNotAvailable { .. } => {
                "network"
            }
            Self::RateLimited { .. }
            | Self::QuotaExceeded { .. }
            | Self::ContextLengthExceeded { .. } => "limit",
            Self::ModelNotFound { .. } => "model",
            Self::Authentication { .. }
            | Self::AuthRequired { .. }
            | Self::TokenExpired { .. }
            | Self::InvalidApiKey { .. }
            | Self::PermissionDenied { .. } => "auth",
            Self::Execution { .. }
            | Self::ToolExecution { .. }
            | Self::ToolNotFound { .. }
            | Self::Sandbox { .. } => "execution",
            Self::ToolBlocked { .. }
            | Self::ToolApprovalRequired { .. }
            | Self::ToolApprovalDenied { .. }
            | Self::CommandBlocked { .. } => "approval",
            Self::Timeout { .. }
            | Self::ChannelClosed
            | Self::InvalidStateTransition { .. }
            | Self::Session { .. }
            | Self::SessionNotFound { .. }
            | Self::ConversationNotFound { .. }
            | Self::Cancelled => "session",
            Self::McpConnection { .. }
            | Self::McpServerNotFound { .. }
            | Self::McpTimeout { .. }
            | Self::McpProtocol { .. } => "mcp",
            Self::PolicyViolation { .. }
            | Self::PolicyNotFound { .. }
            | Self::ApprovalTimeout { .. }
            | Self::ApprovalDenied => "policy",
            Self::Git { .. } | Self::NotInGitRepo | Self::GitOperation { .. } => "git",
            Self::Context { .. } | Self::History { .. } | Self::Rollout { .. } => "context",
            Self::NotFound { .. } | Self::AlreadyExists { .. } | Self::ResourceBusy { .. } => {
                "resource"
            }
            Self::Internal { .. }
            | Self::NotImplemented { .. }
            | Self::InvalidArgument { .. }
            | Self::Hook { .. } => "internal",
        }
    }
}
