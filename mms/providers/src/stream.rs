use crate::message::{Message, ToolCall};
use futures::Stream;
use std::pin::Pin;

pub type ResponseStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, StreamError>> + Send>>;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Start(StreamStart),
    Delta(StreamDelta),
    ToolCallStart(ToolCallStart),
    ToolCallDelta(ToolCallDelta),
    ToolCallEnd(ToolCallEnd),
    End(StreamEnd),
}

#[derive(Debug, Clone)]
pub struct StreamStart {
    pub id: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallStart {
    pub index: usize,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ToolCallDelta {
    pub index: usize,
    pub arguments_delta: String,
}

#[derive(Debug, Clone)]
pub struct ToolCallEnd {
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct StreamEnd {
    pub finish_reason: FinishReason,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
}

#[derive(Debug, Clone)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct StreamError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub id: String,
    pub model: String,
    pub message: Message,
    pub finish_reason: FinishReason,
    pub usage: Option<Usage>,
}

impl StreamDelta {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            reasoning: None,
        }
    }

    pub fn reasoning(reasoning: impl Into<String>) -> Self {
        Self {
            content: None,
            reasoning: Some(reasoning.into()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_none() && self.reasoning.is_none()
    }
}

impl StreamError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }
}

impl Usage {
    pub fn new(input: u64, output: u64) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        }
    }
}

impl From<String> for StreamError {
    fn from(message: String) -> Self {
        Self::new("unknown", message)
    }
}
