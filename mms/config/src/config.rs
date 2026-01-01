use mms_common::AgentResult;
use mms_linux_sandbox::{DiskAccess, NetworkAccess, SandboxPolicy};
use mms_protocol::{ApprovalMode, SessionConfig};
use mms_providers::{Provider, ProviderConfig as ProviderSpec, ProviderKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::features::Features;
use crate::provider::ProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mms_home: PathBuf,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub provider_id: String,
    pub providers: HashMap<String, ProviderConfig>,
    pub approval_mode: ApprovalMode,
    pub timeout_ms: u64,
    pub features: Features,
    pub tools: ToolsConfig,
    #[serde(default)]
    pub sandbox: SandboxConfig,
    /// Custom instructions to add to system prompt
    #[serde(default)]
    pub custom_instructions: Option<String>,
}

impl Config {
    pub fn get_provider(&self) -> AgentResult<&ProviderConfig> {
        self.providers
            .get(&self.provider_id)
            .ok_or_else(|| mms_common::AgentError::not_found(&self.provider_id))
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

    /// Build a Provider instance from the current configuration
    pub fn build_provider(&self) -> AgentResult<Provider> {
        let config = self.get_provider()?;

        // Determine the provider kind from the ID or base_url
        let kind = match self.provider_id.to_lowercase().as_str() {
            "openai" => ProviderKind::OpenAI,
            "anthropic" | "claude" => ProviderKind::Anthropic,
            "groq" => ProviderKind::Groq,
            "ollama" => ProviderKind::Ollama,
            _ => {
                // Try to guess from base_url
                if config.base_url.contains("openai.com") {
                    ProviderKind::OpenAI
                } else if config.base_url.contains("anthropic.com") {
                    ProviderKind::Anthropic
                } else if config.base_url.contains("groq.com") {
                    ProviderKind::Groq
                } else {
                    ProviderKind::Custom
                }
            }
        };

        let spec = ProviderSpec::new(kind)
            .with_base_url(&config.base_url)
            .with_model(&config.default_model)
            .with_timeout(self.timeout_ms / 1000);

        Ok(Provider::new(spec))
    }
}

impl Default for Config {
    fn default() -> Self {
        let mms_home = dirs::home_dir()
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
            mms_home,
            cwd: std::env::current_dir().unwrap_or_default(),
            model: None,
            provider_id: "openai".into(),
            providers,
            approval_mode: ApprovalMode::default(),
            timeout_ms: mms_common::DEFAULT_TIMEOUT_MS,
            features: Features::default(),
            tools: ToolsConfig::default(),
            sandbox: SandboxConfig::default(),
            custom_instructions: None,
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
            max_output_bytes: mms_common::MAX_TOOL_OUTPUT_BYTES,
        }
    }
}

/// Sandbox configuration for command execution security
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Enable sandboxing (Linux Landlock/seccomp)
    pub enabled: bool,
    /// Network access policy
    pub network: NetworkAccessConfig,
    /// Disk read access policy
    pub disk_read: DiskAccessConfig,
    /// Disk write access policy
    pub disk_write: DiskAccessConfig,
    /// Additional writable paths
    #[serde(default)]
    pub writable_paths: Vec<PathBuf>,
    /// Additional readable paths
    #[serde(default)]
    pub readable_paths: Vec<PathBuf>,
    /// Path to exec policy file (TOML)
    pub exec_policy_file: Option<PathBuf>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            network: NetworkAccessConfig::None,
            disk_read: DiskAccessConfig::Full,
            disk_write: DiskAccessConfig::Restricted,
            writable_paths: Vec::new(),
            readable_paths: Vec::new(),
            exec_policy_file: None,
        }
    }
}

impl SandboxConfig {
    /// Convert to a SandboxPolicy
    pub fn to_sandbox_policy(&self, cwd: &PathBuf) -> SandboxPolicy {
        let mut policy = SandboxPolicy::default()
            .with_network(self.network.into())
            .with_disk_read(self.disk_read.into())
            .with_disk_write(self.disk_write.into());

        // Add cwd as writable by default
        policy = policy.add_writable_root(cwd);

        // Add configured writable paths
        for path in &self.writable_paths {
            policy = policy.add_writable_root(path);
        }

        // Add configured readable paths
        for path in &self.readable_paths {
            policy = policy.add_readable_root(path);
        }

        policy
    }

    /// Create a permissive sandbox config (for testing)
    pub fn permissive() -> Self {
        Self {
            enabled: false,
            network: NetworkAccessConfig::Full,
            disk_read: DiskAccessConfig::Full,
            disk_write: DiskAccessConfig::Full,
            writable_paths: Vec::new(),
            readable_paths: Vec::new(),
            exec_policy_file: None,
        }
    }

    /// Create a restrictive sandbox config
    pub fn restrictive() -> Self {
        Self {
            enabled: true,
            network: NetworkAccessConfig::None,
            disk_read: DiskAccessConfig::Restricted,
            disk_write: DiskAccessConfig::Restricted,
            writable_paths: Vec::new(),
            readable_paths: Vec::new(),
            exec_policy_file: None,
        }
    }
}

/// Network access configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccessConfig {
    /// No network access
    #[default]
    None,
    /// Unix sockets only (IPC)
    UnixOnly,
    /// Full network access
    Full,
}

impl From<NetworkAccessConfig> for NetworkAccess {
    fn from(config: NetworkAccessConfig) -> Self {
        match config {
            NetworkAccessConfig::None => NetworkAccess::None,
            NetworkAccessConfig::UnixOnly => NetworkAccess::UnixOnly,
            NetworkAccessConfig::Full => NetworkAccess::Full,
        }
    }
}

/// Disk access configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiskAccessConfig {
    /// No disk access
    None,
    /// Read-only access
    ReadOnly,
    /// Restricted access (configured paths only)
    #[default]
    Restricted,
    /// Full disk access
    Full,
}

impl From<DiskAccessConfig> for DiskAccess {
    fn from(config: DiskAccessConfig) -> Self {
        match config {
            DiskAccessConfig::None => DiskAccess::None,
            DiskAccessConfig::ReadOnly => DiskAccess::ReadOnly,
            DiskAccessConfig::Restricted => DiskAccess::Restricted,
            DiskAccessConfig::Full => DiskAccess::Full,
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
