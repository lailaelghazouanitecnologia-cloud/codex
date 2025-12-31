use agent_common::AgentResult;
use agent_protocol::{ApprovalMode, SessionConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::features::Features;
use crate::provider::ProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent_home: PathBuf,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub provider_id: String,
    pub providers: HashMap<String, ProviderConfig>,
    pub approval_mode: ApprovalMode,
    pub timeout_ms: u64,
    pub features: Features,
    pub tools: ToolsConfig,
}

impl Config {
    pub fn get_provider(&self) -> AgentResult<&ProviderConfig> {
        self.providers
            .get(&self.provider_id)
            .ok_or_else(|| agent_common::AgentError::not_found(&self.provider_id))
    }

    pub fn to_session_config(&self) -> SessionConfig {
        SessionConfig {
            model: self.model.clone().unwrap_or_else(|| "default".into()),
            provider: self.provider_id.clone(),
            cwd: self.cwd.clone(),
            approval_mode: self.approval_mode,
            timeout_ms: self.timeout_ms,
        }
    }

    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = cwd;
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_provider(mut self, provider_id: impl Into<String>) -> Self {
        self.provider_id = provider_id.into();
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        let agent_home = dirs::home_dir()
            .map(|h| h.join(".agent"))
            .unwrap_or_else(|| PathBuf::from(".agent"));

        let mut providers = HashMap::new();
        providers.insert(
            "openai".into(),
            ProviderConfig {
                name: "OpenAI".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_key_env: Some("OPENAI_API_KEY".into()),
                default_model: "gpt-4".into(),
            },
        );

        Self {
            agent_home,
            cwd: std::env::current_dir().unwrap_or_default(),
            model: None,
            provider_id: "openai".into(),
            providers,
            approval_mode: ApprovalMode::default(),
            timeout_ms: agent_common::DEFAULT_TIMEOUT_MS,
            features: Features::default(),
            tools: ToolsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub shell_enabled: bool,
    pub file_read_enabled: bool,
    pub file_write_enabled: bool,
    pub web_search_enabled: bool,
    pub max_output_bytes: usize,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            shell_enabled: true,
            file_read_enabled: true,
            file_write_enabled: true,
            web_search_enabled: false,
            max_output_bytes: agent_common::MAX_TOOL_OUTPUT_BYTES,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub approval_mode: Option<ApprovalMode>,
    pub timeout_ms: Option<u64>,
}

impl ConfigOverrides {
    pub fn apply(self, config: &mut Config) {
        if let Some(model) = self.model {
            config.model = Some(model);
        }
        if let Some(provider_id) = self.provider_id {
            config.provider_id = provider_id;
        }
        if let Some(cwd) = self.cwd {
            config.cwd = cwd;
        }
        if let Some(approval_mode) = self.approval_mode {
            config.approval_mode = approval_mode;
        }
        if let Some(timeout_ms) = self.timeout_ms {
            config.timeout_ms = timeout_ms;
        }
    }
}
