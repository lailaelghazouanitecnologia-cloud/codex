mod provider;
mod client;
mod message;
mod stream;
pub mod clients;

pub use provider::{Provider, ProviderConfig, ProviderKind};
pub use client::{ModelClient, ClientConfig, CompletionRequest};
pub use message::{Message, MessageRole, MessageContent, ToolCall, ToolResult, FunctionCall, ToolDefinition};
pub use stream::{StreamEvent, StreamDelta, ResponseStream, CompletionResponse, FinishReason, Usage, StreamError};
