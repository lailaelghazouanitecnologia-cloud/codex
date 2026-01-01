use mms_common::AgentResult;
use std::collections::HashMap;
use std::sync::Arc;

use crate::skill::{Skill, SkillContext, SkillOutput, SkillSpec};

pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn register<S: Skill + 'static>(&mut self, skill: S) {
        let spec = skill.spec();
        self.skills.insert(spec.name.clone(), Arc::new(skill));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.get(name).cloned()
    }

    pub fn list(&self) -> Vec<SkillSpec> {
        self.skills
            .values()
            .filter(|s| s.is_enabled())
            .map(|s| s.spec())
            .collect()
    }

    pub fn list_names(&self) -> Vec<String> {
        self.skills
            .values()
            .filter(|s| s.is_enabled())
            .map(|s| s.spec().name)
            .collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        ctx: &SkillContext,
        input: serde_json::Value,
    ) -> AgentResult<SkillOutput> {
        let skill = self
            .get(name)
            .ok_or_else(|| mms_common::AgentError::not_found(format!("Skill not found: {}", name)))?;

        skill.execute(ctx, input).await
    }

    pub fn count(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
