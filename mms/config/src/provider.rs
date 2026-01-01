use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub default_model: String,
}

impl ProviderConfig {
    pub fn get_api_key(&self) -> Option<String> {
        self.api_key_env
            .as_ref()
            .and_then(|env_var| std::env::var(env_var).ok())
    }

    pub fn has_api_key(&self) -> bool {
        self.get_api_key().is_some()
    }
}
