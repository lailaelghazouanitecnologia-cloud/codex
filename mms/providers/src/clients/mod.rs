mod openai;
mod anthropic;
mod groq;

pub use openai::OpenAIClient;
pub use anthropic::AnthropicClient;
pub use groq::GroqClient;

use crate::client::ModelClient;
use crate::provider::{Provider, ProviderKind};
use crate::stream::StreamError;

pub fn create_client(provider: Provider) -> Result<Box<dyn ModelClient>, StreamError> {
    match provider.kind() {
        ProviderKind::OpenAI => Ok(Box::new(OpenAIClient::new(provider))),
        ProviderKind::Anthropic => Ok(Box::new(AnthropicClient::new(provider))),
        ProviderKind::Groq => Ok(Box::new(GroqClient::new(provider))),
        ProviderKind::Ollama => Ok(Box::new(OpenAIClient::new(provider))),
        ProviderKind::Custom => Ok(Box::new(OpenAIClient::new(provider))),
    }
}
