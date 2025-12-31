use crate::error::AgentError;

pub type AgentResult<T> = Result<T, AgentError>;
