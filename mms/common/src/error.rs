use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
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

    #[error("network error: {message}")]
    Network { message: String },

    #[error("execution error: {message}")]
    Execution { message: String },

    #[error("tool execution failed: {tool_name} - {message}")]
    ToolExecution { tool_name: String, message: String },

    #[error("timeout after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("channel closed unexpectedly")]
    ChannelClosed,

    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("resource not found: {resource}")]
    NotFound { resource: String },

    #[error("operation cancelled")]
    Cancelled,

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl AgentError {
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

    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

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

    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    pub fn invalid_state(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::InvalidStateTransition {
            from: from.into(),
            to: to.into(),
        }
    }

    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Timeout { .. } => true,
            Self::ChannelClosed => false,
            Self::Cancelled => true,
            Self::Internal { .. } => false,
            _ => true,
        }
    }
}
