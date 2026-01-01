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

pub struct GroqClient {
    provider: Provider,
    http: Client,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCallJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCallJson {
    id: String,
    r#type: String,
    function: FunctionJson,
}

#[derive(Debug, Serialize, Deserialize)]
struct FunctionJson {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    id: String,
    model: String,
    choices: Vec<Choice>,
    usage: Option<UsageJson>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallJson>>,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageJson {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    id: String,
    model: String,
    choices: Vec<StreamChoice>,
    usage: Option<UsageJson>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: DeltaJson,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeltaJson {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDeltaJson>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDeltaJson {
    index: usize,
    id: Option<String>,
    function: Option<FunctionDeltaJson>,
}

#[derive(Debug, Deserialize)]
struct FunctionDeltaJson {
    name: Option<String>,
    arguments: Option<String>,
}

impl GroqClient {
    pub fn new(provider: Provider) -> Self {
        let http = Client::builder()
            .timeout(provider.timeout())
            .build()
            .unwrap_or_default();

        Self { provider, http }
    }

    fn build_request(&self, request: &CompletionRequest) -> ChatRequest {
        let messages: Vec<ChatMessage> = request
            .messages
            .iter()
            .map(|m| self.convert_message(m))
            .collect();

        let tools: Vec<serde_json::Value> = request
            .config
            .tools
            .iter()
            .filter_map(|t| serde_json::to_value(t).ok())
            .collect();

        ChatRequest {
            model: request.config.model.clone(),
            messages,
            temperature: request.config.temperature,
            max_tokens: request.config.max_tokens,
            top_p: request.config.top_p,
            tools,
            stream: request.config.stream,
        }
    }

    fn convert_message(&self, msg: &Message) -> ChatMessage {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        let content = match &msg.content {
            MessageContent::Text(s) => serde_json::Value::String(s.clone()),
            MessageContent::Parts(parts) => {
                serde_json::to_value(parts).unwrap_or(serde_json::Value::Null)
            }
        };

        let tool_calls = msg.tool_calls.as_ref().map(|calls| {
            calls
                .iter()
                .map(|c| ToolCallJson {
                    id: c.id.clone(),
                    r#type: c.r#type.clone(),
                    function: FunctionJson {
                        name: c.function.name.clone(),
                        arguments: c.function.arguments.clone(),
                    },
                })
                .collect()
        });

        ChatMessage {
            role: role.to_string(),
            content,
            tool_calls,
            tool_call_id: msg.tool_call_id.clone(),
        }
    }

    fn parse_finish_reason(reason: Option<&str>) -> FinishReason {
        match reason {
            Some("stop") => FinishReason::Stop,
            Some("tool_calls") => FinishReason::ToolCalls,
            Some("length") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        }
    }

    async fn do_request(&self, chat_request: ChatRequest) -> Result<reqwest::Response, StreamError> {
        let url = format!("{}/chat/completions", self.provider.base_url());

        let mut req = self.http.post(&url).json(&chat_request);

        if let Some(key) = self.provider.api_key() {
            req = req.bearer_auth(key);
        }

        req.send()
            .await
            .map_err(|e| StreamError::new("request_failed", e.to_string()))
    }
}

#[async_trait]
impl ModelClient for GroqClient {
    fn provider(&self) -> &Provider {
        &self.provider
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, StreamError> {
        let mut chat_request = self.build_request(&request);
        chat_request.stream = false;

        let response = self.do_request(chat_request).await?;

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| StreamError::new("parse_error", e.to_string()))?;

        let choice = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| StreamError::new("no_choices", "No choices in response"))?;

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|c| ToolCall {
                    id: c.id,
                    r#type: c.r#type,
                    function: FunctionCall {
                        name: c.function.name,
                        arguments: c.function.arguments,
                    },
                })
                .collect()
        });

        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Text(choice.message.content.unwrap_or_default()),
            name: None,
            tool_calls,
            tool_call_id: None,
        };

        let usage = chat_response.usage.map(|u| Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(CompletionResponse {
            id: chat_response.id,
            model: chat_response.model,
            message,
            finish_reason: Self::parse_finish_reason(choice.finish_reason.as_deref()),
            usage,
        })
    }

    async fn stream(&self, request: CompletionRequest) -> Result<ResponseStream, StreamError> {
        let mut chat_request = self.build_request(&request);
        chat_request.stream = true;

        let response = self.do_request(chat_request).await?;

        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let byte_stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();
            let mut started = false;

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

                    if data == "[DONE]" {
                        continue;
                    }

                    let chunk: StreamChunk = match serde_json::from_str(data) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    if !started {
                        let _ = tx
                            .send(Ok(StreamEvent::Start(StreamStart {
                                id: chunk.id.clone(),
                                model: chunk.model.clone(),
                            })))
                            .await;
                        started = true;
                    }

                    for choice in chunk.choices {
                        if let Some(content) = choice.delta.content {
                            if !content.is_empty() {
                                let _ = tx
                                    .send(Ok(StreamEvent::Delta(StreamDelta::text(content))))
                                    .await;
                            }
                        }

                        if let Some(tool_calls) = choice.delta.tool_calls {
                            for tc in tool_calls {
                                if let Some(id) = tc.id {
                                    let name = tc
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.name.clone())
                                        .unwrap_or_default();
                                    let _ = tx
                                        .send(Ok(StreamEvent::ToolCallStart(ToolCallStart {
                                            index: tc.index,
                                            id,
                                            name,
                                        })))
                                        .await;
                                }

                                if let Some(args) = tc.function.and_then(|f| f.arguments) {
                                    if !args.is_empty() {
                                        let _ = tx
                                            .send(Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
                                                index: tc.index,
                                                arguments_delta: args,
                                            })))
                                            .await;
                                    }
                                }
                            }
                        }

                        if let Some(reason) = choice.finish_reason {
                            let finish_reason = match reason.as_str() {
                                "stop" => FinishReason::Stop,
                                "tool_calls" => FinishReason::ToolCalls,
                                "length" => FinishReason::Length,
                                _ => FinishReason::Stop,
                            };

                            let _ = tx
                                .send(Ok(StreamEvent::End(StreamEnd {
                                    finish_reason,
                                    usage: chunk.usage.clone().map(|u| Usage {
                                        input_tokens: u.prompt_tokens,
                                        output_tokens: u.completion_tokens,
                                        total_tokens: u.total_tokens,
                                    }),
                                })))
                                .await;
                        }
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
