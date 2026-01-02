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
//!
//! # Security
//!
//! - Git trust validation ensures repo skills come from trusted repositories
//! - Boundary checking prevents walking up past repository root
//! - Symlinks are skipped to avoid following external paths

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

use crate::model::{SkillMetadata, SkillScope};

/// Maximum length for skill name.
const MAX_NAME_LENGTH: usize = 64;

/// Maximum length for skill description.
const MAX_DESCRIPTION_LENGTH: usize = 1024;

/// Skill file name that must exist in each skill directory.
const SKILL_FILE_NAME: &str = "SKILL.md";

/// Repository config directory name.
const REPO_CONFIG_DIR_NAME: &str = ".codex";

/// Skills subdirectory name.
const SKILLS_DIR_NAME: &str = "skills";

/// Admin skills root (Unix only).
#[cfg(unix)]
const ADMIN_SKILLS_ROOT: &str = "/etc/codex/skills";

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

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
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

/// A skill search root with its scope.
#[derive(Debug, Clone)]
pub struct SkillRoot {
    /// Path to search for skills.
    pub path: PathBuf,
    /// Scope of skills found here.
    pub scope: SkillScope,
}

/// Skill loader that discovers and loads skills from the filesystem.
pub struct SkillLoader {
    /// Skill search roots in priority order.
    search_roots: Vec<SkillRoot>,
}

impl SkillLoader {
    /// Create a new skill loader with default search roots.
    ///
    /// Searches in priority order: Repo > User > System > Admin
    pub fn new(repo_root: Option<&Path>, home_dir: Option<&Path>) -> Self {
        let mut search_roots = Vec::new();

        // Repo scope (highest priority)
        if let Some(root) = repo_root {
            let repo_skills = root.join(REPO_CONFIG_DIR_NAME).join(SKILLS_DIR_NAME);
            search_roots.push(SkillRoot {
                path: repo_skills,
                scope: SkillScope::Repo,
            });
        }

        // User scope
        if let Some(home) = home_dir {
            let user_skills = home.join(".codex").join(SKILLS_DIR_NAME);
            search_roots.push(SkillRoot {
                path: user_skills.clone(),
                scope: SkillScope::User,
            });

            // System scope (embedded skills)
            let system_skills = user_skills.join(".system");
            search_roots.push(SkillRoot {
                path: system_skills,
                scope: SkillScope::System,
            });
        }

        // Admin scope (Unix only)
        #[cfg(unix)]
        {
            search_roots.push(SkillRoot {
                path: PathBuf::from(ADMIN_SKILLS_ROOT),
                scope: SkillScope::Admin,
            });
        }

        Self { search_roots }
    }

    /// Create a loader for a specific working directory.
    ///
    /// This finds the git repository root and sets up proper boundary checking.
    pub fn for_cwd(cwd: &Path, home_dir: Option<&Path>) -> Self {
        let repo_root = find_repo_skills_root(cwd);
        Self::new(repo_root.as_deref(), home_dir)
    }

    /// Create a loader with custom search roots.
    pub fn with_roots(roots: Vec<SkillRoot>) -> Self {
        Self { search_roots: roots }
    }

    /// Load all skills from all search roots.
    ///
    /// Skills are deduplicated by name, with higher priority scopes winning.
    pub fn load_all(&self) -> SkillLoadOutcome {
        let mut outcome = SkillLoadOutcome::default();
        let mut seen_names: HashSet<String> = HashSet::new();

        for root in &self.search_roots {
            if !root.path.exists() {
                debug!("Skill search root does not exist: {:?}", root.path);
                continue;
            }

            let root_outcome = self.load_from_directory(&root.path, root.scope);

            // Deduplicate by name (keep first occurrence = highest priority)
            for skill in root_outcome.skills {
                if seen_names.contains(&skill.name) {
                    debug!(
                        "Skipping duplicate skill '{}' from {:?}",
                        skill.name, root.scope
                    );
                } else {
                    seen_names.insert(skill.name.clone());
                    outcome.skills.push(skill);
                }
            }

            outcome.errors.extend(root_outcome.errors);
        }

        // Sort by name for consistent ordering
        outcome.skills.sort_by(|a, b| a.name.cmp(&b.name));

        outcome
    }

    /// Load skills from a specific directory.
    pub fn load_from_directory(&self, dir: &Path, scope: SkillScope) -> SkillLoadOutcome {
        let mut outcome = SkillLoadOutcome::default();

        // Normalize path to handle relative paths
        let dir = match normalize_path(dir) {
            Some(p) => p,
            None => {
                debug!("Could not normalize path: {:?}", dir);
                return outcome;
            }
        };

        if !dir.exists() || !dir.is_dir() {
            return outcome;
        }

        // Breadth-first traversal of skill directories
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        queue.push_back(dir.clone());

        while let Some(current) = queue.pop_front() {
            let skill_file = current.join(SKILL_FILE_NAME);

            if skill_file.exists() && skill_file.is_file() {
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
                match std::fs::read_dir(&current) {
                    Ok(entries) => {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let path = entry.path();

                            // Skip symlinks for security
                            if path.is_symlink() {
                                debug!("Skipping symlink: {:?}", path);
                                continue;
                            }

                            // Skip hidden directories (except .codex)
                            if is_hidden_dir(&path) {
                                continue;
                            }

                            if path.is_dir() {
                                queue.push_back(path);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read directory {:?}: {}", current, e);
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
        // Validate name characters (alphanumeric, hyphen, underscore)
        if !frontmatter
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Skill name must contain only alphanumeric characters, hyphens, or underscores".to_string());
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

        // Validate short description if present
        if let Some(ref short) = frontmatter.metadata.short_description {
            if short.len() > MAX_DESCRIPTION_LENGTH {
                return Err(format!(
                    "Short description exceeds maximum length of {} characters",
                    MAX_DESCRIPTION_LENGTH
                ));
            }
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

/// Find the repository skills root for a given working directory.
///
/// Walks up the directory tree looking for `.codex/skills/` directories,
/// but respects git repository boundaries.
fn find_repo_skills_root(cwd: &Path) -> Option<PathBuf> {
    let base = if cwd.is_dir() {
        cwd.to_path_buf()
    } else {
        cwd.parent()?.to_path_buf()
    };

    let base = normalize_path(&base)?;

    // Find git root for boundary checking
    let git_root = find_git_root(&base);

    // Walk up looking for .codex/skills/
    for dir in base.ancestors() {
        let skills_dir = dir.join(REPO_CONFIG_DIR_NAME).join(SKILLS_DIR_NAME);

        if skills_dir.is_dir() {
            // Found a skills directory
            debug!("Found repo skills at {:?}", skills_dir);
            return Some(dir.to_path_buf());
        }

        // Don't walk past git root
        if let Some(ref root) = git_root {
            if dir == root.as_path() {
                debug!("Reached git root boundary at {:?}", root);
                break;
            }
        }
    }

    None
}

/// Find the git repository root from a starting path.
fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

/// Resolve git repository root for trust validation.
///
/// This handles git worktrees by following the gitdir file.
pub fn resolve_git_root_for_trust(path: &Path) -> Option<PathBuf> {
    let git_root = find_git_root(path)?;
    let git_dir = git_root.join(".git");

    // Handle worktrees: .git may be a file pointing to the actual git dir
    if git_dir.is_file() {
        if let Ok(content) = std::fs::read_to_string(&git_dir) {
            if let Some(gitdir) = content.strip_prefix("gitdir: ") {
                let gitdir = gitdir.trim();
                let gitdir_path = if Path::new(gitdir).is_absolute() {
                    PathBuf::from(gitdir)
                } else {
                    git_root.join(gitdir)
                };

                // Go up from .git/worktrees/xxx to get main repo
                if let Some(parent) = gitdir_path.parent() {
                    if let Some(grandparent) = parent.parent() {
                        if let Some(common) = grandparent.parent() {
                            return Some(common.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    Some(git_root)
}

/// Normalize a path (resolve symlinks, canonicalize).
fn normalize_path(path: &Path) -> Option<PathBuf> {
    // Try to canonicalize, fall back to the original path
    std::fs::canonicalize(path).ok().or_else(|| {
        if path.exists() {
            Some(path.to_path_buf())
        } else {
            None
        }
    })
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
    fn test_parse_frontmatter_unclosed() {
        let content = "---\nname: test\n# No closing delimiter";
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

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

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
        create_skill_file(
            &repo_skill,
            r#"---
name: shared-skill
description: Repo version
---
"#,
        );

        // Lower priority (User scope)
        let user_dir = temp_dir.path().join("user");
        let user_skill = user_dir.join("shared-skill");
        create_skill_file(
            &user_skill,
            r#"---
name: shared-skill
description: User version
---
"#,
        );

        let loader = SkillLoader::with_roots(vec![
            SkillRoot {
                path: repo_dir,
                scope: SkillScope::Repo,
            },
            SkillRoot {
                path: user_dir,
                scope: SkillScope::User,
            },
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
        create_skill_file(
            &skill_dir,
            r#"---
description: A skill without a name
---
"#,
        );

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 0);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_invalid_name_characters() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("bad-skill");

        create_skill_file(
            &skill_dir,
            r#"---
name: "bad skill name!"
description: Has invalid characters
---
"#,
        );

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 0);
        assert!(outcome.has_errors());
        assert!(outcome.errors[0].message.contains("alphanumeric"));
    }

    #[test]
    fn test_name_too_long() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("long-skill");

        let long_name = "a".repeat(100);
        create_skill_file(
            &skill_dir,
            &format!(
                r#"---
name: {}
description: Name is too long
---
"#,
                long_name
            ),
        );

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 0);
        assert!(outcome.has_errors());
        assert!(outcome.errors[0].message.contains("maximum length"));
    }

    #[test]
    fn test_skips_hidden_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Visible skill
        let visible = temp_dir.path().join("visible-skill");
        create_skill_file(
            &visible,
            r#"---
name: visible
description: Should be loaded
---
"#,
        );

        // Hidden skill (should be skipped)
        let hidden = temp_dir.path().join(".hidden-skill");
        create_skill_file(
            &hidden,
            r#"---
name: hidden
description: Should be skipped
---
"#,
        );

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 1);
        assert_eq!(outcome.skills[0].name, "visible");
    }

    #[test]
    fn test_git_root_boundary() {
        let temp_dir = TempDir::new().unwrap();

        // Create a fake git repo
        let repo = temp_dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();

        // Create skills inside repo
        let repo_skills = repo.join(".codex").join("skills").join("repo-skill");
        create_skill_file(
            &repo_skills,
            r#"---
name: repo-skill
description: Inside repo
---
"#,
        );

        // Create skills outside repo (should not be found)
        let outside = temp_dir.path().join(".codex").join("skills").join("outside");
        create_skill_file(
            &outside,
            r#"---
name: outside-skill
description: Outside repo
---
"#,
        );

        // Find repo root from a subdirectory
        let subdir = repo.join("src");
        std::fs::create_dir_all(&subdir).unwrap();

        let result = find_repo_skills_root(&subdir);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), repo);
    }

    #[test]
    fn test_multiple_skills_sorted() {
        let temp_dir = TempDir::new().unwrap();

        // Create skills in non-alphabetical order
        for name in ["charlie", "alpha", "beta"] {
            let skill_dir = temp_dir.path().join(name);
            create_skill_file(
                &skill_dir,
                &format!(
                    r#"---
name: {}
description: Skill {}
---
"#,
                    name, name
                ),
            );
        }

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 3);

        // Should be sorted alphabetically
        assert_eq!(outcome.skills[0].name, "alpha");
        assert_eq!(outcome.skills[1].name, "beta");
        assert_eq!(outcome.skills[2].name, "charlie");
    }

    #[test]
    fn test_nested_skill_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested skills
        let skill1 = temp_dir.path().join("category1").join("skill1");
        create_skill_file(
            &skill1,
            r#"---
name: skill1
description: Nested skill 1
---
"#,
        );

        let skill2 = temp_dir.path().join("category2").join("subcategory").join("skill2");
        create_skill_file(
            &skill2,
            r#"---
name: skill2
description: Deeply nested skill
---
"#,
        );

        let loader = SkillLoader::with_roots(vec![SkillRoot {
            path: temp_dir.path().to_path_buf(),
            scope: SkillScope::User,
        }]);

        let outcome = loader.load_all();
        assert_eq!(outcome.skill_count(), 2);
    }

    #[test]
    fn test_for_cwd_constructor() {
        let temp_dir = TempDir::new().unwrap();

        // Create a git repo with skills
        let repo = temp_dir.path().join("project");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let skills = repo.join(".codex").join("skills").join("test-skill");
        create_skill_file(
            &skills,
            r#"---
name: test-skill
description: Test
---
"#,
        );

        let loader = SkillLoader::for_cwd(&repo, None);
        let outcome = loader.load_all();

        // Should find the repo skill
        assert_eq!(outcome.skill_count(), 1);
        assert_eq!(outcome.skills[0].name, "test-skill");
    }
}
