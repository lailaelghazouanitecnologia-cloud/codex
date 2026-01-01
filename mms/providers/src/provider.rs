use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Groq,
    Ollama,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub default_model: String,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Provider {
    pub config: ProviderConfig,
    api_key: Option<String>,
}

impl ProviderKind {
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::Ollama => "http://localhost:11434/v1",
            Self::Custom => "",
        }
    }

    pub fn default_env_key(&self) -> Option<&'static str> {
        match self {
            Self::OpenAI => Some("OPENAI_API_KEY"),
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::Groq => Some("GROQ_API_KEY"),
            Self::Ollama => None,
            Self::Custom => None,
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Self::OpenAI => "gpt-4o",
            Self::Anthropic => "claude-sonnet-4-20250514",
            Self::Groq => "llama-3.3-70b-versatile",
            Self::Ollama => "llama3.2",
            Self::Custom => "default",
        }
    }
}

impl ProviderConfig {
    pub fn new(kind: ProviderKind) -> Self {
        Self {
            kind,
            name: format!("{:?}", kind),
            base_url: kind.default_base_url().to_string(),
            api_key_env: kind.default_env_key().map(String::from),
            default_model: kind.default_model().to_string(),
            max_retries: 3,
            timeout_seconds: 120,
            headers: HashMap::new(),
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

impl Provider {
    pub fn new(config: ProviderConfig) -> Self {
        let api_key = config
            .api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok());

        Self { config, api_key }
    }

    pub fn openai() -> Self {
        Self::new(ProviderConfig::new(ProviderKind::OpenAI))
    }

    pub fn anthropic() -> Self {
        Self::new(ProviderConfig::new(ProviderKind::Anthropic))
    }

    pub fn groq() -> Self {
        Self::new(ProviderConfig::new(ProviderKind::Groq))
    }

    pub fn ollama() -> Self {
        Self::new(ProviderConfig::new(ProviderKind::Ollama))
    }

    pub fn kind(&self) -> ProviderKind {
        self.config.kind
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub fn default_model(&self) -> &str {
        &self.config.default_model
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.config.timeout_seconds)
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self::new(ProviderKind::OpenAI)
    }
}
