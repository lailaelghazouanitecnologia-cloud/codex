//! Provider-specific authentication handling.
//!
//! This module provides provider-specific auth configuration
//! for different AI model providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Wire API protocol for a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireApi {
    /// OpenAI Responses API (newer).
    #[default]
    Responses,

    /// OpenAI Chat Completions API.
    Chat,

    /// Anthropic Messages API.
    Messages,
}

/// Model provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Display name for the provider.
    pub name: String,

    /// Provider ID (e.g., "openai", "anthropic").
    #[serde(default)]
    pub id: String,

    /// Base URL for API requests.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Environment variable name for API key.
    #[serde(default)]
    pub env_key: Option<String>,

    /// Instructions for obtaining API key.
    #[serde(default)]
    pub env_key_instructions: Option<String>,

    /// Wire API protocol to use.
    #[serde(default)]
    pub wire_api: WireApi,

    /// Additional query parameters for API requests.
    #[serde(default)]
    pub query_params: Option<HashMap<String, String>>,

    /// Static HTTP headers for API requests.
    #[serde(default)]
    pub http_headers: Option<HashMap<String, String>>,

    /// Environment-based HTTP headers (header name -> env var name).
    #[serde(default)]
    pub env_http_headers: Option<HashMap<String, String>>,

    /// Maximum retries for request failures.
    #[serde(default)]
    pub request_max_retries: Option<u64>,

    /// Maximum retries for stream failures.
    #[serde(default)]
    pub stream_max_retries: Option<u64>,

    /// Timeout for idle stream in milliseconds.
    #[serde(default)]
    pub stream_idle_timeout_ms: Option<u64>,

    /// Whether this provider requires OpenAI OAuth login.
    #[serde(default)]
    pub requires_openai_auth: bool,

    /// Whether this provider is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            id: String::new(),
            base_url: None,
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::default(),
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            enabled: true,
        }
    }
}

impl ProviderConfig {
    /// Create an OpenAI provider configuration.
    pub fn openai() -> Self {
        Self {
            name: "OpenAI".to_string(),
            id: "openai".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get your API key from https://platform.openai.com/api-keys".to_string(),
            ),
            wire_api: WireApi::Responses,
            requires_openai_auth: false,
            ..Default::default()
        }
    }

    /// Create an OpenAI ChatGPT provider configuration (OAuth).
    pub fn chatgpt() -> Self {
        Self {
            name: "ChatGPT".to_string(),
            id: "chatgpt".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: None,
            wire_api: WireApi::Responses,
            requires_openai_auth: true,
            ..Default::default()
        }
    }

    /// Create an Anthropic provider configuration.
    pub fn anthropic() -> Self {
        Self {
            name: "Anthropic".to_string(),
            id: "anthropic".to_string(),
            base_url: Some("https://api.anthropic.com/v1".to_string()),
            env_key: Some("ANTHROPIC_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get your API key from https://console.anthropic.com/settings/keys".to_string(),
            ),
            wire_api: WireApi::Messages,
            ..Default::default()
        }
    }

    /// Create an Ollama provider configuration.
    pub fn ollama() -> Self {
        Self {
            name: "Ollama".to_string(),
            id: "ollama".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            env_key: None,
            wire_api: WireApi::Chat,
            ..Default::default()
        }
    }

    /// Create an LM Studio provider configuration.
    pub fn lm_studio() -> Self {
        Self {
            name: "LM Studio".to_string(),
            id: "lm-studio".to_string(),
            base_url: Some("http://localhost:1234/v1".to_string()),
            env_key: None,
            wire_api: WireApi::Chat,
            ..Default::default()
        }
    }

    /// Create a Groq provider configuration.
    pub fn groq() -> Self {
        Self {
            name: "Groq".to_string(),
            id: "groq".to_string(),
            base_url: Some("https://api.groq.com/openai/v1".to_string()),
            env_key: Some("GROQ_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get your API key from https://console.groq.com/keys".to_string(),
            ),
            wire_api: WireApi::Chat,
            ..Default::default()
        }
    }

    /// Create a Together AI provider configuration.
    pub fn together() -> Self {
        Self {
            name: "Together AI".to_string(),
            id: "together".to_string(),
            base_url: Some("https://api.together.xyz/v1".to_string()),
            env_key: Some("TOGETHER_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get your API key from https://api.together.xyz/settings/api-keys".to_string(),
            ),
            wire_api: WireApi::Chat,
            ..Default::default()
        }
    }

    /// Create a Fireworks AI provider configuration.
    pub fn fireworks() -> Self {
        Self {
            name: "Fireworks AI".to_string(),
            id: "fireworks".to_string(),
            base_url: Some("https://api.fireworks.ai/inference/v1".to_string()),
            env_key: Some("FIREWORKS_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get your API key from https://fireworks.ai/account/api-keys".to_string(),
            ),
            wire_api: WireApi::Chat,
            ..Default::default()
        }
    }

    /// Create a Mistral AI provider configuration.
    pub fn mistral() -> Self {
        Self {
            name: "Mistral AI".to_string(),
            id: "mistral".to_string(),
            base_url: Some("https://api.mistral.ai/v1".to_string()),
            env_key: Some("MISTRAL_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get your API key from https://console.mistral.ai/api-keys".to_string(),
            ),
            wire_api: WireApi::Chat,
            ..Default::default()
        }
    }

    /// Get all builtin provider configurations.
    pub fn builtins() -> Vec<ProviderConfig> {
        vec![
            Self::openai(),
            Self::chatgpt(),
            Self::anthropic(),
            Self::ollama(),
            Self::lm_studio(),
            Self::groq(),
            Self::together(),
            Self::fireworks(),
            Self::mistral(),
        ]
    }

    /// Get a provider by ID.
    pub fn by_id(id: &str) -> Option<ProviderConfig> {
        Self::builtins().into_iter().find(|p| p.id == id)
    }

    /// Check if the provider is a local provider (no auth needed).
    pub fn is_local(&self) -> bool {
        matches!(self.id.as_str(), "ollama" | "lm-studio")
    }

    /// Get the API key from environment.
    pub fn api_key_from_env(&self) -> Option<String> {
        self.env_key.as_ref().and_then(|key| std::env::var(key).ok())
    }

    /// Check if auth is required for this provider.
    pub fn requires_auth(&self) -> bool {
        !self.is_local() && !self.requires_openai_auth
    }
}

/// Authentication status for a specific provider.
#[derive(Debug, Clone)]
pub struct ProviderAuthStatus {
    /// Provider configuration.
    pub provider: ProviderConfig,

    /// Whether the provider is authenticated.
    pub authenticated: bool,

    /// Source of authentication (env, file, oauth).
    pub auth_source: Option<AuthSource>,

    /// Error message if authentication check failed.
    pub error: Option<String>,
}

/// Source of authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSource {
    /// From environment variable.
    Environment,

    /// From stored file.
    File,

    /// From keyring.
    Keyring,

    /// From OAuth login.
    OAuth,
}

impl ProviderAuthStatus {
    /// Create an unauthenticated status.
    pub fn unauthenticated(provider: ProviderConfig) -> Self {
        Self {
            provider,
            authenticated: false,
            auth_source: None,
            error: None,
        }
    }

    /// Create an authenticated status.
    pub fn authenticated(provider: ProviderConfig, source: AuthSource) -> Self {
        Self {
            provider,
            authenticated: true,
            auth_source: Some(source),
            error: None,
        }
    }

    /// Create an error status.
    pub fn error(provider: ProviderConfig, error: String) -> Self {
        Self {
            provider,
            authenticated: false,
            auth_source: None,
            error: Some(error),
        }
    }
}

/// Check authentication status for a provider.
pub fn check_provider_auth(config: &ProviderConfig) -> ProviderAuthStatus {
    // Local providers don't need auth
    if config.is_local() {
        return ProviderAuthStatus::authenticated(config.clone(), AuthSource::Environment);
    }

    // Check environment variable
    if let Some(ref env_key) = config.env_key {
        if std::env::var(env_key).is_ok() {
            return ProviderAuthStatus::authenticated(config.clone(), AuthSource::Environment);
        }
    }

    // OAuth providers need special handling
    if config.requires_openai_auth {
        // This would need to check OAuth token storage
        return ProviderAuthStatus::unauthenticated(config.clone());
    }

    ProviderAuthStatus::unauthenticated(config.clone())
}

/// Check authentication status for all providers.
pub fn check_all_providers_auth() -> Vec<ProviderAuthStatus> {
    ProviderConfig::builtins()
        .iter()
        .map(check_provider_auth)
        .collect()
}
