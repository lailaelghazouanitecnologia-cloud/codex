//! Core skill data models.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Scope/origin of a skill (determines priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    /// Repository-level skill (.codex/skills/).
    Repo,
    /// User-level skill (~/.codex/skills/).
    User,
    /// System-level embedded skill (~/.codex/skills/.system/).
    System,
    /// Admin-level skill (/etc/codex/skills/).
    Admin,
}

impl SkillScope {
    /// Get the priority of this scope (higher = more priority).
    pub fn priority(&self) -> u8 {
        match self {
            Self::Repo => 4,
            Self::User => 3,
            Self::System => 2,
            Self::Admin => 1,
        }
    }

    /// Get a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Repo => "Repository",
            Self::User => "User",
            Self::System => "System",
            Self::Admin => "Admin",
        }
    }
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Metadata for a loaded skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Unique skill name.
    pub name: String,

    /// Description of when to use this skill.
    pub description: String,

    /// Short description for UI display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,

    /// Path to the skill directory.
    pub path: PathBuf,

    /// Scope/origin of the skill.
    pub scope: SkillScope,

    /// Version of the skill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Author of the skill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Tags for categorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Body content of the skill (markdown instructions).
    #[serde(skip)]
    pub body: String,
}

impl SkillMetadata {
    /// Get display name (short description if available, otherwise name).
    pub fn display_name(&self) -> &str {
        self.short_description.as_deref().unwrap_or(&self.name)
    }

    /// Get the skill file path (SKILL.md).
    pub fn skill_file(&self) -> PathBuf {
        self.path.join("SKILL.md")
    }

    /// Check if this skill has resource directories.
    pub fn has_resources(&self) -> bool {
        self.path.join("scripts").exists()
            || self.path.join("references").exists()
            || self.path.join("assets").exists()
    }

    /// Get scripts directory if it exists.
    pub fn scripts_dir(&self) -> Option<PathBuf> {
        let dir = self.path.join("scripts");
        if dir.exists() {
            Some(dir)
        } else {
            None
        }
    }

    /// Get references directory if it exists.
    pub fn references_dir(&self) -> Option<PathBuf> {
        let dir = self.path.join("references");
        if dir.exists() {
            Some(dir)
        } else {
            None
        }
    }

    /// Get assets directory if it exists.
    pub fn assets_dir(&self) -> Option<PathBuf> {
        let dir = self.path.join("assets");
        if dir.exists() {
            Some(dir)
        } else {
            None
        }
    }
}

/// Skill contents ready for injection into conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstructions {
    /// Skill name.
    pub name: String,

    /// Path to the skill (for display).
    pub path: String,

    /// Full contents of the SKILL.md file.
    pub contents: String,
}

impl SkillInstructions {
    /// Create from skill metadata by loading file contents.
    pub fn from_metadata(meta: &SkillMetadata) -> Result<Self, std::io::Error> {
        let contents = std::fs::read_to_string(meta.skill_file())?;
        Ok(Self {
            name: meta.name.clone(),
            path: meta.path.display().to_string(),
            contents,
        })
    }

    /// Format for injection into conversation context.
    ///
    /// Wraps skill in XML tags for clear delineation.
    pub fn to_injection_format(&self) -> String {
        format!(
            "<skill>\n<name>{}</name>\n<path>{}</path>\n<instructions>\n{}\n</instructions>\n</skill>",
            self.name, self.path, self.contents
        )
    }
}

/// Summary of skills available in a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsSummary {
    /// Total number of skills available.
    pub total: usize,

    /// Skills by scope.
    pub by_scope: std::collections::HashMap<SkillScope, usize>,

    /// Any errors during loading.
    pub load_errors: usize,
}

impl SkillsSummary {
    /// Create from skill metadata list.
    pub fn from_skills(skills: &[SkillMetadata], error_count: usize) -> Self {
        let mut by_scope = std::collections::HashMap::new();

        for skill in skills {
            *by_scope.entry(skill.scope).or_insert(0) += 1;
        }

        Self {
            total: skills.len(),
            by_scope,
            load_errors: error_count,
        }
    }

    /// Get a human-readable summary.
    pub fn to_string(&self) -> String {
        let mut parts = vec![format!("{} skills available", self.total)];

        if self.load_errors > 0 {
            parts.push(format!("({} load errors)", self.load_errors));
        }

        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_scope_priority() {
        assert!(SkillScope::Repo.priority() > SkillScope::User.priority());
        assert!(SkillScope::User.priority() > SkillScope::System.priority());
        assert!(SkillScope::System.priority() > SkillScope::Admin.priority());
    }

    #[test]
    fn test_skill_instructions_format() {
        let instr = SkillInstructions {
            name: "test-skill".to_string(),
            path: "/path/to/skill".to_string(),
            contents: "# Instructions\nDo something.".to_string(),
        };

        let formatted = instr.to_injection_format();
        assert!(formatted.contains("<skill>"));
        assert!(formatted.contains("<name>test-skill</name>"));
        assert!(formatted.contains("# Instructions"));
        assert!(formatted.contains("</skill>"));
    }
}
