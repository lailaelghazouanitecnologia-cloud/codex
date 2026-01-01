use async_trait::async_trait;
use mms_common::AgentResult;
use mms_providers::Provider;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::vector::{Embedding, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    async fn embed(&self, request: EmbeddingRequest) -> AgentResult<EmbeddingResponse>;

    async fn embed_single(&self, text: &str) -> AgentResult<Embedding> {
        let request = EmbeddingRequest::single(text, self.default_model());
        let response = self.embed(request).await?;
        response
            .embeddings
            .into_iter()
            .next()
            .ok_or_else(|| mms_common::AgentError::not_found("No embedding in response"))
    }

    fn default_model(&self) -> String;
}

pub struct OpenAIEmbeddingClient {
    provider: Provider,
    http: Client,
}

#[derive(Debug, Serialize)]
struct OpenAIEmbeddingRequest {
    input: Vec<String>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
    model: String,
    usage: OpenAIEmbeddingUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
    #[allow(dead_code)]
    index: usize,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingUsage {
    prompt_tokens: u64,
    total_tokens: u64,
}

impl OpenAIEmbeddingClient {
    pub fn new(provider: Provider) -> Self {
        let http = Client::builder()
            .timeout(provider.timeout())
            .build()
            .unwrap_or_default();

        Self { provider, http }
    }
}

#[async_trait]
impl EmbeddingClient for OpenAIEmbeddingClient {
    async fn embed(&self, request: EmbeddingRequest) -> AgentResult<EmbeddingResponse> {
        let url = format!("{}/embeddings", self.provider.base_url());

        let api_request = OpenAIEmbeddingRequest {
            input: request.input,
            model: request.model,
        };

        let mut req = self.http.post(&url).json(&api_request);

        if let Some(key) = self.provider.api_key() {
            req = req.bearer_auth(key);
        }

        let response = req
            .send()
            .await
            .map_err(|e| mms_common::AgentError::network(e.to_string()))?;

        let api_response: OpenAIEmbeddingResponse = response
            .json()
            .await
            .map_err(|e| mms_common::AgentError::parse(e.to_string()))?;

        let embeddings = api_response
            .data
            .into_iter()
            .map(|d| Embedding::new(d.embedding))
            .collect();

        Ok(EmbeddingResponse {
            embeddings,
            model: api_response.model,
            usage: EmbeddingUsage {
                prompt_tokens: api_response.usage.prompt_tokens,
                total_tokens: api_response.usage.total_tokens,
            },
        })
    }

    fn default_model(&self) -> String {
        "text-embedding-3-small".to_string()
    }
}
