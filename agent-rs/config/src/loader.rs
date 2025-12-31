use agent_common::{AgentError, AgentResult};
use std::path::{Path, PathBuf};

use crate::config::{Config, ConfigOverrides};

const CONFIG_FILE_NAME: &str = "config.toml";

pub struct ConfigLoader {
    agent_home: PathBuf,
}

impl ConfigLoader {
    pub fn new(agent_home: PathBuf) -> Self {
        Self { agent_home }
    }

    pub fn from_default_home() -> AgentResult<Self> {
        let home = find_agent_home()?;
        Ok(Self::new(home))
    }

    pub fn load(&self) -> AgentResult<Config> {
        self.load_with_overrides(ConfigOverrides::default())
    }

    pub fn load_with_overrides(&self, overrides: ConfigOverrides) -> AgentResult<Config> {
        let config_path = self.agent_home.join(CONFIG_FILE_NAME);

        let mut config = if config_path.exists() {
            self.load_from_file(&config_path)?
        } else {
            Config::default()
        };

        config.agent_home = self.agent_home.clone();
        overrides.apply(&mut config);

        Ok(config)
    }

    fn load_from_file(&self, path: &Path) -> AgentResult<Config> {
        let content = std::fs::read_to_string(path)?;

        toml::from_str(&content).map_err(|e| AgentError::config(e.to_string()))
    }
}

pub fn find_agent_home() -> AgentResult<PathBuf> {
    if let Ok(path) = std::env::var("AGENT_HOME") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Ok(path);
        }
    }

    dirs::home_dir()
        .map(|h| h.join(".agent"))
        .ok_or_else(|| AgentError::config("could not determine home directory"))
}

pub fn ensure_agent_home() -> AgentResult<PathBuf> {
    let home = find_agent_home()?;

    if !home.exists() {
        std::fs::create_dir_all(&home)?;
    }

    Ok(home)
}
