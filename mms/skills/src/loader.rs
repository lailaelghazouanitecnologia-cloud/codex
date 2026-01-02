//! Skill loader for discovering and loading skills from markdown files.
//!
//! Skills are loaded from `SKILL.md` files in skill directories.
//! Each skill directory can contain:
//! - `SKILL.md` - Required, contains skill definition with YAML frontmatter
//! - `scripts/` - Optional executable scripts
//! - `references/` - Optional documentation files
//! - `assets/` - Optional templates and resources
//!
//! # Skill Scopes (Priority Order)
//!
//! 1. **Repo** - `.codex/skills/` within git repository root
//! 2. **User** - `$HOME/.codex/skills/`
//! 3. **System** - `$HOME/.codex/skills/.system/`
//! 4. **Admin** - `/etc/codex/skills/` (Unix only)
//!
//! Skills are deduplicated by name, with higher priority scopes winning.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::model::{SkillMetadata, SkillScope};

/// Maximum length for skill name.
const MAX_NAME_LENGTH: usize = 64;

/// Maximum length for skill description.
const MAX_DESCRIPTION_LENGTH: usize = 1024;

/// Skill file name that must exist in each skill directory.
const SKILL_FILE_NAME: &str = "SKILL.md";

/// Errors that can occur during skill loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillError {
    /// Path where the error occurred.
    pub path: PathBuf,
    /// Error message.
    pub message: String,
}

impl SkillError {
    pub fn new(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Result of loading skills from a directory tree.
#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    /// Successfully loaded skills.
    pub skills: Vec<SkillMetadata>,
    /// Errors encountered during loading.
    pub errors: Vec<SkillError>,
}

impl SkillLoadOutcome {
    /// Check if any skills were loaded.
    pub fn has_skills(&self) -> bool {
        !self.skills.is_empty()
    }

    /// Check if any errors occurred.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get skill count.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Merge another outcome into this one.
    pub fn merge(&mut self, other: SkillLoadOutcome) {
        self.skills.extend(other.skills);
        self.errors.extend(other.errors);
    }
}

/// YAML frontmatter structure for SKILL.md files.
#[derive(Debug, Clone, Deserialize)]
struct SkillFrontmatter {
    /// Skill name (required).
    name: String,
    /// When to use this skill (required).
    description: String,
    /// Optional metadata.
    #[serde(default)]
    metadata: SkillMetadataYaml,
}

/// Optional metadata in skill frontmatter.
#[derive(Debug, Clone, Default, Deserialize)]
struct SkillMetadataYaml {
    /// Short description for UI display.
    #[serde(rename = "short-description")]
    short_description: Option<String>,
    /// Version of the skill.
    version: Option<String>,
    /// Author of the skill.
    author: Option<String>,
    /// Tags for categorization.
    #[serde(default)]
    tags: Vec<String>,
}

/// Skill loader that discovers and loads skills from the filesystem.
pub struct SkillLoader {
    /// Skill search roots in priority order.
    search_roots: Vec<(PathBuf, SkillScope)>,
}

impl SkillLoader {
    /// Create a new skill loader with default search roots.
    pub fn new(repo_root: Option<&Path>, home_dir: Option<&Path>) -> Self {
        let mut search_roots = Vec::new();

        // Repo scope (highest priority)
        if let Some(root) = repo_root {
            let repo_skills = root.join(".codex").join("skills");
            search_roots.push((repo_skills, SkillScope::Repo));
        }

        // User scope
        if let Some(home) = home_dir {
            let user_skills = home.join(".codex").join("skills");
            search_roots.push((user_skills.clone(), SkillScope::User));

            // System scope (embedded skills)
            let system_skills = user_skills.join(".system");
            search_roots.push((system_skills, SkillScope::System));
        }

        // Admin scope (Unix only)
        #[cfg(unix)]
        {
            let admin_skills = PathBuf::from("/etc/codex/skills");
            search_roots.push((admin_skills, SkillScope::Admin));
        }

        Self { search_roots }
    }

    /// Create a loader with custom search roots.
    pub fn with_roots(roots: Vec<(PathBuf, SkillScope)>) -> Self {
        Self { search_roots: roots }
    }

    /// Load all skills from all search roots.
    ///
    /// Skills are deduplicated by name, with higher priority scopes winning.
    pub fn load_all(&self) -> SkillLoadOutcome {
        let mut outcome = SkillLoadOutcome::default();
        let mut seen_names: HashMap<String, SkillScope> = HashMap::new();

        for (root, scope) in &self.search_roots {
            if !root.exists() {
                debug!("Skill search root does not exist: {:?}", root);
                continue;
            }

            let root_outcome = self.load_from_directory(root, *scope);

            // Deduplicate by name (keep first occurrence = highest priority)
            for skill in root_outcome.skills {
                if let Some(existing_scope) = seen_names.get(&skill.name) {
                    debug!(
                        "Skipping duplicate skill '{}' from {:?} (already loaded from {:?})",
                        skill.name, scope, existing_scope
                    );
                } else {
                    seen_names.insert(skill.name.clone(), *scope);
                    outcome.skills.push(skill);
                }
            }

            outcome.errors.extend(root_outcome.errors);
        }

        outcome
    }

    /// Load skills from a specific directory.
    pub fn load_from_directory(&self, dir: &Path, scope: SkillScope) -> SkillLoadOutcome {
        let mut outcome = SkillLoadOutcome::default();

        if !dir.exists() || !dir.is_dir() {
            return outcome;
        }

        // Breadth-first traversal of skill directories
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        queue.push_back(dir.to_path_buf());

        while let Some(current) = queue.pop_front() {
            let skill_file = current.join(SKILL_FILE_NAME);

            if skill_file.exists() {
                // Found a skill directory
                match self.load_skill(&skill_file, scope) {
                    Ok(skill) => {
                        debug!("Loaded skill '{}' from {:?}", skill.name, skill_file);
                        outcome.skills.push(skill);
                    }
                    Err(e) => {
                        warn!("Failed to load skill from {:?}: {}", skill_file, e);
                        outcome.errors.push(SkillError::new(skill_file, e));
                    }
                }
            } else {
                // Not a skill directory, check subdirectories
                if let Ok(entries) = std::fs::read_dir(&current) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_dir() && !is_hidden_dir(&path) {
                            queue.push_back(path);
                        }
                    }
                }
            }
        }

        outcome
    }

    /// Load a single skill from a SKILL.md file.
    fn load_skill(&self, path: &Path, scope: SkillScope) -> Result<SkillMetadata, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let (frontmatter, body) = parse_frontmatter(&content)?;

        // Validate name
        if frontmatter.name.is_empty() {
            return Err("Skill name is required".to_string());
        }
        if frontmatter.name.len() > MAX_NAME_LENGTH {
            return Err(format!(
                "Skill name exceeds maximum length of {} characters",
                MAX_NAME_LENGTH
            ));
        }

        // Validate description
        if frontmatter.description.is_empty() {
            return Err("Skill description is required".to_string());
        }
        if frontmatter.description.len() > MAX_DESCRIPTION_LENGTH {
            return Err(format!(
                "Skill description exceeds maximum length of {} characters",
                MAX_DESCRIPTION_LENGTH
            ));
        }

        // Get skill directory (parent of SKILL.md)
        let skill_dir = path.parent().unwrap_or(path).to_path_buf();

        Ok(SkillMetadata {
            name: frontmatter.name,
            description: frontmatter.description,
            short_description: frontmatter.metadata.short_description,
            path: skill_dir,
            scope,
            version: frontmatter.metadata.version,
            author: frontmatter.metadata.author,
            tags: frontmatter.metadata.tags,
            body: body.to_string(),
        })
    }

    /// Get skill contents by reading the full SKILL.md file.
    pub fn load_skill_contents(&self, skill: &SkillMetadata) -> Result<String, String> {
        let skill_file = skill.path.join(SKILL_FILE_NAME);
        std::fs::read_to_string(&skill_file)
            .map_err(|e| format!("Failed to read skill file: {}", e))
    }
}

/// Parse YAML frontmatter from markdown content.
fn parse_frontmatter(content: &str) -> Result<(SkillFrontmatter, &str), String> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Err("Missing YAML frontmatter (must start with ---)".to_string());
    }

    let after_first = &content[3..];
    let end_index = after_first
        .find("\n---")
        .ok_or("Missing closing frontmatter delimiter (---)")?;

    let yaml_content = &after_first[..end_index].trim();
    let body_start = 3 + end_index + 4; // Skip "---\n" at start and "\n---" at end
    let body = if body_start < content.len() {
        content[body_start..].trim()
    } else {
        ""
    };

    let frontmatter: SkillFrontmatter =
        serde_yaml::from_str(yaml_content).map_err(|e| format!("Invalid YAML frontmatter: {}", e))?;

    Ok((frontmatter, body))
}

/// Check if a directory is hidden (starts with dot but is not .codex).
fn is_hidden_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.') && n != ".codex")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_skill_file(dir: &Path, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = r#"---
name: test-skill
description: A test skill for testing
metadata:
  short-description: Test skill
  version: "1.0"
  tags:
    - test
    - example
---
# Test Skill

This is the body of the skill.
"#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill for testing");
        assert_eq!(fm.metadata.short_description, Some("Test skill".to_string()));
        assert_eq!(fm.metadata.version, Some("1.0".to_string()));
        assert_eq!(fm.metadata.tags, vec!["test", "example"]);
        assert!(body.contains("# Test Skill"));
    }

    #[test]
    fn test_parse_frontmatter_missing() {
        let content = "# No frontmatter";
        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_skill_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("my-skill");

        let content = r#"---
name: my-skill
description: My test skill
---
Instructions here.
"#;
        create_skill_file(&skill_dir, content);

        let loader = SkillLoader::with_roots(vec![
            (temp_dir.path().to_path_buf(), SkillScope::User),
        ]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 1);
        assert!(!outcome.has_errors());

        let skill = &outcome.skills[0];
        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.description, "My test skill");
        assert_eq!(skill.scope, SkillScope::User);
    }

    #[test]
    fn test_skill_deduplication() {
        let temp_dir = TempDir::new().unwrap();

        // Higher priority (Repo scope)
        let repo_dir = temp_dir.path().join("repo");
        let repo_skill = repo_dir.join("shared-skill");
        create_skill_file(&repo_skill, r#"---
name: shared-skill
description: Repo version
---
"#);

        // Lower priority (User scope)
        let user_dir = temp_dir.path().join("user");
        let user_skill = user_dir.join("shared-skill");
        create_skill_file(&user_skill, r#"---
name: shared-skill
description: User version
---
"#);

        let loader = SkillLoader::with_roots(vec![
            (repo_dir, SkillScope::Repo),
            (user_dir, SkillScope::User),
        ]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 1);

        // Should keep repo version (higher priority)
        let skill = &outcome.skills[0];
        assert_eq!(skill.name, "shared-skill");
        assert_eq!(skill.description, "Repo version");
        assert_eq!(skill.scope, SkillScope::Repo);
    }

    #[test]
    fn test_validation_errors() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("invalid-skill");

        // Missing name
        create_skill_file(&skill_dir, r#"---
description: A skill without a name
---
"#);

        let loader = SkillLoader::with_roots(vec![
            (temp_dir.path().to_path_buf(), SkillScope::User),
        ]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 0);
        assert!(outcome.has_errors());
    }
}
