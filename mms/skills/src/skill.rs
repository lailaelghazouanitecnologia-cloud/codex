use async_trait::async_trait;
use mms_common::AgentResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillContext {
    pub session_id: String,
    pub working_dir: String,
    pub variables: HashMap<String, serde_json::Value>,
}

impl SkillContext {
    pub fn new(session_id: impl Into<String>, working_dir: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            working_dir: working_dir.into(),
            variables: HashMap::new(),
        }
    }

    pub fn set_variable(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.variables.insert(key.into(), value);
    }

    pub fn get_variable(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    pub content: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SkillOutput {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSpec {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

#[async_trait]
pub trait Skill: Send + Sync {
    fn spec(&self) -> SkillSpec;

    async fn execute(
        &self,
        ctx: &SkillContext,
        input: serde_json::Value,
    ) -> AgentResult<SkillOutput>;

    fn is_enabled(&self) -> bool {
        true
    }
}
