use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;

use crate::client::{ClientConfig, CompletionRequest, ModelClient};
use crate::message::{Message, MessageContent, MessageRole, ToolCall, FunctionCall};
use crate::provider::Provider;
use crate::stream::{
    CompletionResponse, FinishReason, ResponseStream, StreamDelta, StreamEnd, StreamError,
    StreamEvent, StreamStart, ToolCallDelta, ToolCallStart, Usage,
};

const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    provider: Provider,
    http: Client,
}

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    id: String,
    model: String,
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct StreamMessage {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    message: Option<MessageStart>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    content_block: Option<ContentBlock>,
    #[serde(default)]
    delta: Option<DeltaBlock>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct MessageStart {
    id: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct DeltaBlock {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
    partial_json: Option<String>,
    stop_reason: Option<String>,
}

impl AnthropicClient {
    pub fn new(provider: Provider) -> Self {
        let http = Client::builder()
            .timeout(provider.timeout())
            .build()
            .unwrap_or_default();

        Self { provider, http }
    }

    fn build_request(&self, request: &CompletionRequest) -> MessagesRequest {
        let mut system_message = None;
        let mut messages = Vec::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    if let MessageContent::Text(text) = &msg.content {
                        system_message = Some(text.clone());
                    }
                }
                MessageRole::User => {
                    messages.push(self.convert_user_message(msg));
                }
                MessageRole::Assistant => {
                    messages.push(self.convert_assistant_message(msg));
                }
                MessageRole::Tool => {
                    messages.push(self.convert_tool_message(msg));
                }
            }
        }

        let tools: Vec<AnthropicTool> = request
            .config
            .tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.function.name.clone(),
                description: t.function.description.clone().unwrap_or_default(),
                input_schema: t.function.parameters.clone().unwrap_or(serde_json::json!({})),
            })
            .collect();

        MessagesRequest {
            model: request.config.model.clone(),
            messages,
            system: system_message,
            max_tokens: request.config.max_tokens.unwrap_or(4096),
            temperature: request.config.temperature,
            top_p: request.config.top_p,
            tools,
            stream: request.config.stream,
        }
    }

    fn convert_user_message(&self, msg: &Message) -> AnthropicMessage {
        let content = match &msg.content {
            MessageContent::Text(text) => AnthropicContent::Text(text.clone()),
            MessageContent::Parts(_) => AnthropicContent::Text(String::new()),
        };

        AnthropicMessage {
            role: "user".to_string(),
            content,
        }
    }

    fn convert_assistant_message(&self, msg: &Message) -> AnthropicMessage {
        let mut blocks = Vec::new();

        if let MessageContent::Text(text) = &msg.content {
            if !text.is_empty() {
                blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }

        if let Some(tool_calls) = &msg.tool_calls {
            for tc in tool_calls {
                let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::json!({}));
                blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                });
            }
        }

        let content = if blocks.len() == 1 {
            if let ContentBlock::Text { text } = &blocks[0] {
                AnthropicContent::Text(text.clone())
            } else {
                AnthropicContent::Blocks(blocks)
            }
        } else {
            AnthropicContent::Blocks(blocks)
        };

        AnthropicMessage {
            role: "assistant".to_string(),
            content,
        }
    }

    fn convert_tool_message(&self, msg: &Message) -> AnthropicMessage {
        let tool_use_id = msg.tool_call_id.clone().unwrap_or_default();
        let content_text = match &msg.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(_) => String::new(),
        };

        AnthropicMessage {
            role: "user".to_string(),
            content: AnthropicContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id,
                content: content_text,
            }]),
        }
    }

    fn parse_finish_reason(reason: Option<&str>) -> FinishReason {
        match reason {
            Some("end_turn") => FinishReason::Stop,
            Some("tool_use") => FinishReason::ToolCalls,
            Some("max_tokens") => FinishReason::Length,
            Some("stop_sequence") => FinishReason::Stop,
            _ => FinishReason::Stop,
        }
    }

    async fn do_request(&self, msg_request: MessagesRequest) -> Result<reqwest::Response, StreamError> {
        let url = format!("{}/messages", self.provider.base_url());

        let mut req = self
            .http
            .post(&url)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&msg_request);

        if let Some(key) = self.provider.api_key() {
            req = req.header("x-api-key", key);
        }

        req.send()
            .await
            .map_err(|e| StreamError::new("request_failed", e.to_string()))
    }
}

#[async_trait]
impl ModelClient for AnthropicClient {
    fn provider(&self) -> &Provider {
        &self.provider
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, StreamError> {
        let mut msg_request = self.build_request(&request);
        msg_request.stream = false;

        let response = self.do_request(msg_request).await?;

        let msg_response: MessagesResponse = response
            .json()
            .await
            .map_err(|e| StreamError::new("parse_error", e.to_string()))?;

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in msg_response.content {
            match block {
                ContentBlock::Text { text } => {
                    text_content.push_str(&text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        r#type: "function".to_string(),
                        function: FunctionCall {
                            name,
                            arguments: serde_json::to_string(&input).unwrap_or_default(),
                        },
                    });
                }
                ContentBlock::ToolResult { .. } => {}
            }
        }

        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Text(text_content),
            name: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            tool_call_id: None,
        };

        let usage = Usage {
            input_tokens: msg_response.usage.input_tokens,
            output_tokens: msg_response.usage.output_tokens,
            total_tokens: msg_response.usage.input_tokens + msg_response.usage.output_tokens,
        };

        Ok(CompletionResponse {
            id: msg_response.id,
            model: msg_response.model,
            message,
            finish_reason: Self::parse_finish_reason(msg_response.stop_reason.as_deref()),
            usage: Some(usage),
        })
    }

    async fn stream(&self, request: CompletionRequest) -> Result<ResponseStream, StreamError> {
        let mut msg_request = self.build_request(&request);
        msg_request.stream = true;

        let response = self.do_request(msg_request).await?;

        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();
            let mut current_tool_index: Option<usize> = None;

            futures::pin_mut!(byte_stream);

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(StreamError::new("stream_error", e.to_string()))).await;
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }

                    let data = &line[6..];
                    let msg: StreamMessage = match serde_json::from_str(data) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    match msg.event_type.as_str() {
                        "message_start" => {
                            if let Some(message) = msg.message {
                                let _ = tx
                                    .send(Ok(StreamEvent::Start(StreamStart {
                                        id: message.id,
                                        model: message.model,
                                    })))
                                    .await;
                            }
                        }
                        "content_block_start" => {
                            if let Some(ContentBlock::ToolUse { id, name, .. }) = msg.content_block {
                                current_tool_index = msg.index;
                                let _ = tx
                                    .send(Ok(StreamEvent::ToolCallStart(ToolCallStart {
                                        index: msg.index.unwrap_or(0),
                                        id,
                                        name,
                                    })))
                                    .await;
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = msg.delta {
                                if let Some(text) = delta.text {
                                    if !text.is_empty() {
                                        let _ = tx
                                            .send(Ok(StreamEvent::Delta(StreamDelta::text(text))))
                                            .await;
                                    }
                                }
                                if let Some(partial_json) = delta.partial_json {
                                    if !partial_json.is_empty() {
                                        let _ = tx
                                            .send(Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
                                                index: current_tool_index.unwrap_or(0),
                                                arguments_delta: partial_json,
                                            })))
                                            .await;
                                    }
                                }
                            }
                        }
                        "message_delta" => {
                            if let Some(delta) = msg.delta {
                                if let Some(stop_reason) = delta.stop_reason {
                                    let finish_reason = Self::parse_finish_reason(Some(&stop_reason));
                                    let usage = msg.usage.map(|u| Usage {
                                        input_tokens: u.input_tokens,
                                        output_tokens: u.output_tokens,
                                        total_tokens: u.input_tokens + u.output_tokens,
                                    });
                                    let _ = tx
                                        .send(Ok(StreamEvent::End(StreamEnd {
                                            finish_reason,
                                            usage,
                                        })))
                                        .await;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn supports_vision(&self) -> bool {
        true
    }
}
