//! Skill manager for loading and caching skills per working directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info};

use crate::loader::{SkillError, SkillLoadOutcome, SkillLoader};
use crate::model::{SkillMetadata, SkillInstructions, SkillsSummary};

/// Information about installed system skills.
#[derive(Debug, Clone)]
pub struct SystemSkillsInfo {
    /// List of installed system skill names.
    pub installed_skills: Vec<String>,

    /// Current installed fingerprint (if any).
    pub current_fingerprint: Option<String>,

    /// Expected fingerprint for embedded skills.
    pub expected_fingerprint: String,

    /// Whether the installed skills are up to date.
    pub is_current: bool,
}

/// Manager for skill loading and caching.
///
/// Caches skills per working directory to avoid repeated filesystem scans.
pub struct SkillManager {
    /// Cache of skills by working directory.
    cache: RwLock<HashMap<PathBuf, CachedSkills>>,

    /// Home directory for user skills.
    home_dir: Option<PathBuf>,

    /// Whether system skills have been installed.
    system_installed: bool,
}

/// Cached skills for a working directory.
#[derive(Clone)]
struct CachedSkills {
    /// Loaded skills.
    skills: Vec<SkillMetadata>,

    /// Errors during loading.
    errors: Vec<SkillError>,
}

impl SkillManager {
    /// Create a new skill manager.
    pub fn new() -> Self {
        let home_dir = dirs::home_dir();

        Self {
            cache: RwLock::new(HashMap::new()),
            home_dir,
            system_installed: false,
        }
    }

    /// Create with a specific home directory (for testing).
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            home_dir: Some(home),
            system_installed: false,
        }
    }

    /// Initialize the manager and install system skills if needed.
    ///
    /// Uses fingerprinting to avoid unnecessary reinstalls - skills are only
    /// reinstalled when the embedded content changes.
    pub fn initialize(&mut self) -> Result<(), std::io::Error> {
        if !self.system_installed {
            self.install_system_skills()?;
            self.system_installed = true;
        }
        Ok(())
    }

    /// Install embedded system skills using fingerprinting.
    ///
    /// This uses the system module's fingerprinting to:
    /// - Skip installation if skills are already up to date
    /// - Reinstall if embedded content has changed
    /// - Track installed version with a marker file
    fn install_system_skills(&self) -> Result<(), std::io::Error> {
        let Some(home) = &self.home_dir else {
            debug!("No home directory, skipping system skill installation");
            return Ok(());
        };

        let codex_home = home.join(".codex");

        match crate::system::install_system_skills(&codex_home) {
            Ok(true) => {
                info!("System skills installed successfully");
            }
            Ok(false) => {
                debug!("System skills already up to date, skipped installation");
            }
            Err(e) => {
                // Log but don't fail - system skills are optional
                tracing::warn!("Failed to install system skills: {}", e);
            }
        }

        Ok(())
    }

    /// Check if system skills are installed and up to date.
    pub fn system_skills_installed(&self) -> bool {
        let Some(home) = &self.home_dir else {
            return false;
        };
        let codex_home = home.join(".codex");
        crate::system::is_installed(&codex_home)
    }

    /// Get information about installed system skills.
    pub fn system_skills_info(&self) -> Option<SystemSkillsInfo> {
        let home = self.home_dir.as_ref()?;
        let codex_home = home.join(".codex");

        let installed = crate::system::list_installed_skills(&codex_home).ok()?;
        let current_fingerprint = crate::system::current_fingerprint(&codex_home);
        let expected_fingerprint = crate::system::expected_fingerprint();
        let is_current = current_fingerprint.as_ref() == Some(&expected_fingerprint);

        Some(SystemSkillsInfo {
            installed_skills: installed,
            current_fingerprint,
            expected_fingerprint,
            is_current,
        })
    }

    /// Get skills for a specific working directory.
    ///
    /// Uses cache if available, otherwise loads from filesystem.
    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(cwd) {
                return SkillLoadOutcome {
                    skills: cached.skills.clone(),
                    errors: cached.errors.clone(),
                };
            }
        }

        // Load from filesystem
        self.load_and_cache(cwd)
    }

    /// Force reload skills for a working directory.
    pub fn reload_skills(&self, cwd: &Path) -> SkillLoadOutcome {
        // Remove from cache
        {
            let mut cache = self.cache.write();
            cache.remove(cwd);
        }

        // Reload
        self.load_and_cache(cwd)
    }

    /// Load skills and cache the result.
    fn load_and_cache(&self, cwd: &Path) -> SkillLoadOutcome {
        // Find git repo root for repo-scoped skills
        let repo_root = find_git_root(cwd);

        let loader = SkillLoader::new(repo_root.as_deref(), self.home_dir.as_deref());
        let outcome = loader.load_all();

        // Cache the result
        {
            let mut cache = self.cache.write();
            cache.insert(
                cwd.to_path_buf(),
                CachedSkills {
                    skills: outcome.skills.clone(),
                    errors: outcome.errors.clone(),
                },
            );
        }

        outcome
    }

    /// Get a specific skill by name.
    pub fn get_skill(&self, cwd: &Path, name: &str) -> Option<SkillMetadata> {
        let outcome = self.skills_for_cwd(cwd);
        outcome.skills.into_iter().find(|s| s.name == name)
    }

    /// Get skills matching a search query.
    pub fn search_skills(&self, cwd: &Path, query: &str) -> Vec<SkillMetadata> {
        let outcome = self.skills_for_cwd(cwd);
        let query_lower = query.to_lowercase();

        outcome
            .skills
            .into_iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Load skill instructions for injection into context.
    pub fn load_instructions(&self, skill: &SkillMetadata) -> Result<SkillInstructions, String> {
        SkillInstructions::from_metadata(skill)
            .map_err(|e| format!("Failed to load skill instructions: {}", e))
    }

    /// Get a summary of available skills.
    pub fn get_summary(&self, cwd: &Path) -> SkillsSummary {
        let outcome = self.skills_for_cwd(cwd);
        SkillsSummary::from_skills(&outcome.skills, outcome.errors.len())
    }

    /// Clear the cache.
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the git repository root from a starting path.
fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

/// Thread-safe shared skill manager.
pub type SharedSkillManager = Arc<RwLock<SkillManager>>;

/// Create a new shared skill manager.
pub fn new_shared_manager() -> SharedSkillManager {
    Arc::new(RwLock::new(SkillManager::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_skill(dir: &Path, name: &str, description: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                r#"---
name: {}
description: {}
---
Instructions for {}.
"#,
                name, description, name
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_skills_for_cwd() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().join("home");
        let user_skills = home.join(".codex").join("skills");
        std::fs::create_dir_all(&user_skills).unwrap();

        create_skill(&user_skills, "test-skill", "A test skill");

        let manager = SkillManager::with_home(home);
        let cwd = temp_dir.path().join("project");
        std::fs::create_dir_all(&cwd).unwrap();

        let outcome = manager.skills_for_cwd(&cwd);
        assert_eq!(outcome.skill_count(), 1);
        assert_eq!(outcome.skills[0].name, "test-skill");
    }

    #[test]
    fn test_cache() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().join("home");
        let user_skills = home.join(".codex").join("skills");
        std::fs::create_dir_all(&user_skills).unwrap();

        create_skill(&user_skills, "cached-skill", "A cached skill");

        let manager = SkillManager::with_home(home);
        let cwd = temp_dir.path();

        // First load
        let outcome1 = manager.skills_for_cwd(cwd);
        assert_eq!(outcome1.skill_count(), 1);

        // Should be cached
        let outcome2 = manager.skills_for_cwd(cwd);
        assert_eq!(outcome2.skill_count(), 1);
    }

    #[test]
    fn test_search_skills() {
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().join("home");
        let user_skills = home.join(".codex").join("skills");
        std::fs::create_dir_all(&user_skills).unwrap();

        create_skill(&user_skills, "code-review", "Review code changes");
        create_skill(&user_skills, "test-helper", "Help with tests");

        let manager = SkillManager::with_home(home);
        let cwd = temp_dir.path();

        let results = manager.search_skills(cwd, "review");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");

        let results = manager.search_skills(cwd, "HELP");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test-helper");
    }
}
