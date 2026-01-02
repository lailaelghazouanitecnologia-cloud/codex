mod provider;
mod client;
mod message;
mod stream;
pub mod clients;
pub mod model_info;

pub use provider::{Provider, ProviderConfig, ProviderKind};
pub use client::{ModelClient, ClientConfig, CompletionRequest};
pub use message::{Message, MessageRole, MessageContent, ToolCall, ToolResult, FunctionCall, ToolDefinition};
pub use stream::{StreamEvent, StreamDelta, ResponseStream, CompletionResponse, FinishReason, Usage, StreamError};
pub use model_info::{
    ModelCapability, ModelInfo, ModelRegistry, PricingTier, ProviderInfo, RateLimits,
};
