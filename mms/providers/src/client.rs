use async_trait::async_trait;
use std::time::Duration;

use crate::message::{Message, ToolDefinition};
use crate::provider::Provider;
use crate::stream::{CompletionResponse, ResponseStream, StreamError};

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub tools: Vec<ToolDefinition>,
    pub parallel_tool_calls: bool,
    pub stream: bool,
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub config: ClientConfig,
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    fn provider(&self) -> &Provider;

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, StreamError>;

    async fn stream(&self, request: CompletionRequest) -> Result<ResponseStream, StreamError>;

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn supports_vision(&self) -> bool {
        false
    }
}

impl ClientConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            tools: Vec::new(),
            parallel_tool_calls: true,
            stream: true,
        }
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

impl CompletionRequest {
    pub fn new(messages: Vec<Message>, config: ClientConfig) -> Self {
        Self { messages, config }
    }

    pub fn simple(messages: Vec<Message>, model: impl Into<String>) -> Self {
        Self {
            messages,
            config: ClientConfig::new(model),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self::new("gpt-4o")
    }
}
