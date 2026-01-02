//! Model information and capabilities for various providers.
//!
//! This module provides detailed information about available models
//! from different providers, including capabilities and limits.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    /// Text generation.
    TextGeneration,
    /// Chat/conversation.
    Chat,
    /// Code generation.
    Code,
    /// Vision/image understanding.
    Vision,
    /// Function/tool calling.
    FunctionCalling,
    /// Structured output (JSON mode).
    StructuredOutput,
    /// Extended thinking/reasoning.
    ExtendedThinking,
    /// Streaming responses.
    Streaming,
    /// System prompts.
    SystemPrompt,
    /// Multi-turn conversation.
    MultiTurn,
    /// Image generation.
    ImageGeneration,
    /// Audio input.
    AudioInput,
    /// Audio output.
    AudioOutput,
    /// Embedding generation.
    Embeddings,
}

/// Model pricing tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingTier {
    /// Free tier.
    Free,
    /// Low cost tier.
    Low,
    /// Standard tier.
    Standard,
    /// Premium tier.
    Premium,
    /// Enterprise tier.
    Enterprise,
}

/// Model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Provider ID.
    pub provider: String,

    /// Model family (e.g., "claude-3", "gpt-4").
    pub family: String,

    /// Model description.
    pub description: String,

    /// Context window size (tokens).
    pub context_window: u64,

    /// Maximum output tokens.
    pub max_output_tokens: u64,

    /// Training data cutoff date.
    pub training_cutoff: Option<String>,

    /// Supported capabilities.
    pub capabilities: Vec<ModelCapability>,

    /// Pricing tier.
    pub pricing_tier: PricingTier,

    /// Input cost per million tokens (USD).
    pub input_cost_per_mtok: Option<f64>,

    /// Output cost per million tokens (USD).
    pub output_cost_per_mtok: Option<f64>,

    /// Whether the model is deprecated.
    pub deprecated: bool,

    /// Replacement model if deprecated.
    pub replacement: Option<String>,

    /// Additional metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ModelInfo {
    /// Check if the model has a specific capability.
    pub fn has_capability(&self, capability: ModelCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Check if the model supports vision.
    pub fn supports_vision(&self) -> bool {
        self.has_capability(ModelCapability::Vision)
    }

    /// Check if the model supports function calling.
    pub fn supports_functions(&self) -> bool {
        self.has_capability(ModelCapability::FunctionCalling)
    }

    /// Check if the model supports extended thinking.
    pub fn supports_extended_thinking(&self) -> bool {
        self.has_capability(ModelCapability::ExtendedThinking)
    }

    /// Check if the model is available (not deprecated).
    pub fn is_available(&self) -> bool {
        !self.deprecated
    }
}

/// Provider information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Provider ID.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Base API URL.
    pub base_url: String,

    /// Documentation URL.
    pub docs_url: String,

    /// Environment variable for API key.
    pub api_key_env: String,

    /// Default model ID.
    pub default_model: String,

    /// Available models.
    pub models: Vec<ModelInfo>,

    /// Whether the provider requires authentication.
    pub requires_auth: bool,

    /// Whether the provider is self-hosted.
    pub self_hosted: bool,

    /// Rate limits.
    pub rate_limits: Option<RateLimits>,
}

/// Rate limit information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Requests per minute.
    pub requests_per_minute: Option<u32>,

    /// Tokens per minute.
    pub tokens_per_minute: Option<u64>,

    /// Requests per day.
    pub requests_per_day: Option<u32>,
}

impl ProviderInfo {
    /// Get a model by ID.
    pub fn get_model(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == model_id)
    }

    /// Get the default model.
    pub fn default_model_info(&self) -> Option<&ModelInfo> {
        self.get_model(&self.default_model)
    }

    /// Get all available (non-deprecated) models.
    pub fn available_models(&self) -> Vec<&ModelInfo> {
        self.models.iter().filter(|m| m.is_available()).collect()
    }

    /// Get models with a specific capability.
    pub fn models_with_capability(&self, capability: ModelCapability) -> Vec<&ModelInfo> {
        self.models
            .iter()
            .filter(|m| m.has_capability(capability) && m.is_available())
            .collect()
    }
}

/// Model registry containing all provider information.
#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    providers: HashMap<String, ProviderInfo>,
}

impl ModelRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with built-in provider information.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_provider(anthropic_provider());
        registry.register_provider(openai_provider());
        registry.register_provider(groq_provider());
        registry.register_provider(ollama_provider());
        registry.register_provider(gemini_provider());
        registry.register_provider(azure_provider());
        registry.register_provider(bedrock_provider());
        registry
    }

    /// Register a provider.
    pub fn register_provider(&mut self, provider: ProviderInfo) {
        self.providers.insert(provider.id.clone(), provider);
    }

    /// Get a provider by ID.
    pub fn get_provider(&self, provider_id: &str) -> Option<&ProviderInfo> {
        self.providers.get(provider_id)
    }

    /// Get a model by provider and model ID.
    pub fn get_model(&self, provider_id: &str, model_id: &str) -> Option<&ModelInfo> {
        self.providers
            .get(provider_id)
            .and_then(|p| p.get_model(model_id))
    }

    /// List all providers.
    pub fn list_providers(&self) -> Vec<&ProviderInfo> {
        self.providers.values().collect()
    }

    /// List all models across all providers.
    pub fn list_all_models(&self) -> Vec<(&ProviderInfo, &ModelInfo)> {
        self.providers
            .values()
            .flat_map(|p| p.models.iter().map(move |m| (p, m)))
            .collect()
    }

    /// Find models matching a query.
    pub fn search_models(&self, query: &str) -> Vec<(&ProviderInfo, &ModelInfo)> {
        let query_lower = query.to_lowercase();
        self.list_all_models()
            .into_iter()
            .filter(|(p, m)| {
                m.id.to_lowercase().contains(&query_lower)
                    || m.name.to_lowercase().contains(&query_lower)
                    || p.name.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
}

// ─── Provider Definitions ───────────────────────────────────────────────────

/// Create Anthropic provider info.
pub fn anthropic_provider() -> ProviderInfo {
    ProviderInfo {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        docs_url: "https://docs.anthropic.com".to_string(),
        api_key_env: "ANTHROPIC_API_KEY".to_string(),
        default_model: "claude-sonnet-4-20250514".to_string(),
        requires_auth: true,
        self_hosted: false,
        rate_limits: Some(RateLimits {
            requests_per_minute: Some(50),
            tokens_per_minute: Some(40000),
            requests_per_day: None,
        }),
        models: vec![
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                name: "Claude Opus 4".to_string(),
                provider: "anthropic".to_string(),
                family: "claude-4".to_string(),
                description: "Most capable Claude model with extended thinking".to_string(),
                context_window: 200000,
                max_output_tokens: 32000,
                training_cutoff: Some("2025-01".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::ExtendedThinking,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Premium,
                input_cost_per_mtok: Some(15.0),
                output_cost_per_mtok: Some(75.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                provider: "anthropic".to_string(),
                family: "claude-4".to_string(),
                description: "Balanced performance and cost".to_string(),
                context_window: 200000,
                max_output_tokens: 16000,
                training_cutoff: Some("2025-01".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::ExtendedThinking,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Standard,
                input_cost_per_mtok: Some(3.0),
                output_cost_per_mtok: Some(15.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                provider: "anthropic".to_string(),
                family: "claude-3.5".to_string(),
                description: "Fast and cost-effective for simple tasks".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
                training_cutoff: Some("2024-04".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Low,
                input_cost_per_mtok: Some(1.0),
                output_cost_per_mtok: Some(5.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

/// Create OpenAI provider info.
pub fn openai_provider() -> ProviderInfo {
    ProviderInfo {
        id: "openai".to_string(),
        name: "OpenAI".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        docs_url: "https://platform.openai.com/docs".to_string(),
        api_key_env: "OPENAI_API_KEY".to_string(),
        default_model: "gpt-4o".to_string(),
        requires_auth: true,
        self_hosted: false,
        rate_limits: Some(RateLimits {
            requests_per_minute: Some(500),
            tokens_per_minute: Some(30000),
            requests_per_day: None,
        }),
        models: vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                provider: "openai".to_string(),
                family: "gpt-4".to_string(),
                description: "Most capable GPT-4 model".to_string(),
                context_window: 128000,
                max_output_tokens: 16384,
                training_cutoff: Some("2024-10".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                    ModelCapability::AudioInput,
                    ModelCapability::AudioOutput,
                ],
                pricing_tier: PricingTier::Standard,
                input_cost_per_mtok: Some(2.5),
                output_cost_per_mtok: Some(10.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "gpt-4o-mini".to_string(),
                name: "GPT-4o Mini".to_string(),
                provider: "openai".to_string(),
                family: "gpt-4".to_string(),
                description: "Smaller, faster GPT-4o variant".to_string(),
                context_window: 128000,
                max_output_tokens: 16384,
                training_cutoff: Some("2024-10".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Low,
                input_cost_per_mtok: Some(0.15),
                output_cost_per_mtok: Some(0.6),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "o1".to_string(),
                name: "o1".to_string(),
                provider: "openai".to_string(),
                family: "o1".to_string(),
                description: "Reasoning model for complex tasks".to_string(),
                context_window: 200000,
                max_output_tokens: 100000,
                training_cutoff: Some("2024-10".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::ExtendedThinking,
                    ModelCapability::Streaming,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Premium,
                input_cost_per_mtok: Some(15.0),
                output_cost_per_mtok: Some(60.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

/// Create Groq provider info.
pub fn groq_provider() -> ProviderInfo {
    ProviderInfo {
        id: "groq".to_string(),
        name: "Groq".to_string(),
        base_url: "https://api.groq.com/openai/v1".to_string(),
        docs_url: "https://console.groq.com/docs".to_string(),
        api_key_env: "GROQ_API_KEY".to_string(),
        default_model: "llama-3.3-70b-versatile".to_string(),
        requires_auth: true,
        self_hosted: false,
        rate_limits: Some(RateLimits {
            requests_per_minute: Some(30),
            tokens_per_minute: Some(6000),
            requests_per_day: Some(14400),
        }),
        models: vec![
            ModelInfo {
                id: "llama-3.3-70b-versatile".to_string(),
                name: "Llama 3.3 70B".to_string(),
                provider: "groq".to_string(),
                family: "llama-3".to_string(),
                description: "Fast inference with Llama 3.3".to_string(),
                context_window: 128000,
                max_output_tokens: 32768,
                training_cutoff: Some("2024-12".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Low,
                input_cost_per_mtok: Some(0.59),
                output_cost_per_mtok: Some(0.79),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "mixtral-8x7b-32768".to_string(),
                name: "Mixtral 8x7B".to_string(),
                provider: "groq".to_string(),
                family: "mixtral".to_string(),
                description: "Efficient mixture of experts model".to_string(),
                context_window: 32768,
                max_output_tokens: 32768,
                training_cutoff: Some("2024-01".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Low,
                input_cost_per_mtok: Some(0.24),
                output_cost_per_mtok: Some(0.24),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

/// Create Ollama provider info.
pub fn ollama_provider() -> ProviderInfo {
    ProviderInfo {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        base_url: "http://localhost:11434/v1".to_string(),
        docs_url: "https://ollama.ai/docs".to_string(),
        api_key_env: "".to_string(),
        default_model: "llama3.2".to_string(),
        requires_auth: false,
        self_hosted: true,
        rate_limits: None,
        models: vec![
            ModelInfo {
                id: "llama3.2".to_string(),
                name: "Llama 3.2".to_string(),
                provider: "ollama".to_string(),
                family: "llama-3".to_string(),
                description: "Local Llama 3.2 model".to_string(),
                context_window: 128000,
                max_output_tokens: 8192,
                training_cutoff: Some("2024-09".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Free,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "codellama".to_string(),
                name: "Code Llama".to_string(),
                provider: "ollama".to_string(),
                family: "llama".to_string(),
                description: "Code-specialized Llama model".to_string(),
                context_window: 16384,
                max_output_tokens: 4096,
                training_cutoff: Some("2024-01".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                ],
                pricing_tier: PricingTier::Free,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "qwen2.5-coder".to_string(),
                name: "Qwen 2.5 Coder".to_string(),
                provider: "ollama".to_string(),
                family: "qwen".to_string(),
                description: "Qwen code generation model".to_string(),
                context_window: 32768,
                max_output_tokens: 8192,
                training_cutoff: Some("2024-09".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Free,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

/// Create Gemini provider info.
pub fn gemini_provider() -> ProviderInfo {
    ProviderInfo {
        id: "gemini".to_string(),
        name: "Google Gemini".to_string(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        docs_url: "https://ai.google.dev/docs".to_string(),
        api_key_env: "GEMINI_API_KEY".to_string(),
        default_model: "gemini-2.0-flash".to_string(),
        requires_auth: true,
        self_hosted: false,
        rate_limits: Some(RateLimits {
            requests_per_minute: Some(60),
            tokens_per_minute: Some(4000000),
            requests_per_day: None,
        }),
        models: vec![
            ModelInfo {
                id: "gemini-2.0-flash".to_string(),
                name: "Gemini 2.0 Flash".to_string(),
                provider: "gemini".to_string(),
                family: "gemini-2".to_string(),
                description: "Fast multimodal model".to_string(),
                context_window: 1000000,
                max_output_tokens: 8192,
                training_cutoff: Some("2024-08".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                    ModelCapability::AudioInput,
                ],
                pricing_tier: PricingTier::Low,
                input_cost_per_mtok: Some(0.075),
                output_cost_per_mtok: Some(0.3),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "gemini-2.0-pro".to_string(),
                name: "Gemini 2.0 Pro".to_string(),
                provider: "gemini".to_string(),
                family: "gemini-2".to_string(),
                description: "Advanced reasoning capabilities".to_string(),
                context_window: 2000000,
                max_output_tokens: 8192,
                training_cutoff: Some("2024-08".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::ExtendedThinking,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Standard,
                input_cost_per_mtok: Some(1.25),
                output_cost_per_mtok: Some(5.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

/// Create Azure OpenAI provider info.
pub fn azure_provider() -> ProviderInfo {
    ProviderInfo {
        id: "azure".to_string(),
        name: "Azure OpenAI".to_string(),
        base_url: "https://{resource}.openai.azure.com".to_string(),
        docs_url: "https://learn.microsoft.com/en-us/azure/ai-services/openai/".to_string(),
        api_key_env: "AZURE_OPENAI_API_KEY".to_string(),
        default_model: "gpt-4o".to_string(),
        requires_auth: true,
        self_hosted: false,
        rate_limits: None, // Depends on deployment
        models: vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o (Azure)".to_string(),
                provider: "azure".to_string(),
                family: "gpt-4".to_string(),
                description: "GPT-4o on Azure infrastructure".to_string(),
                context_window: 128000,
                max_output_tokens: 16384,
                training_cutoff: Some("2024-10".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::StructuredOutput,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Standard,
                input_cost_per_mtok: None, // Varies by deployment
                output_cost_per_mtok: None,
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

/// Create AWS Bedrock provider info.
pub fn bedrock_provider() -> ProviderInfo {
    ProviderInfo {
        id: "bedrock".to_string(),
        name: "AWS Bedrock".to_string(),
        base_url: "https://bedrock-runtime.{region}.amazonaws.com".to_string(),
        docs_url: "https://docs.aws.amazon.com/bedrock/".to_string(),
        api_key_env: "AWS_ACCESS_KEY_ID".to_string(),
        default_model: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        requires_auth: true,
        self_hosted: false,
        rate_limits: None, // Depends on account
        models: vec![
            ModelInfo {
                id: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
                name: "Claude 3.5 Sonnet (Bedrock)".to_string(),
                provider: "bedrock".to_string(),
                family: "claude-3.5".to_string(),
                description: "Claude 3.5 Sonnet on AWS Bedrock".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
                training_cutoff: Some("2024-04".to_string()),
                capabilities: vec![
                    ModelCapability::Chat,
                    ModelCapability::Code,
                    ModelCapability::Vision,
                    ModelCapability::FunctionCalling,
                    ModelCapability::Streaming,
                    ModelCapability::SystemPrompt,
                    ModelCapability::MultiTurn,
                ],
                pricing_tier: PricingTier::Standard,
                input_cost_per_mtok: Some(3.0),
                output_cost_per_mtok: Some(15.0),
                deprecated: false,
                replacement: None,
                metadata: HashMap::new(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_defaults() {
        let registry = ModelRegistry::with_defaults();
        assert!(registry.get_provider("anthropic").is_some());
        assert!(registry.get_provider("openai").is_some());
    }

    #[test]
    fn test_model_lookup() {
        let registry = ModelRegistry::with_defaults();
        let model = registry.get_model("anthropic", "claude-sonnet-4-20250514");
        assert!(model.is_some());
        assert!(model.map(|m| m.has_capability(ModelCapability::Vision)).unwrap_or(false));
    }

    #[test]
    fn test_search_models() {
        let registry = ModelRegistry::with_defaults();
        let results = registry.search_models("claude");
        assert!(!results.is_empty());
    }
}
