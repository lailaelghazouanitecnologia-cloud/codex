//! System skill installation with fingerprinting.
//!
//! This module handles installation of embedded system skills to the user's
//! skill directory with hash-based fingerprinting to avoid unnecessary reinstalls.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use tracing::{debug, info};

use crate::builtin;

/// Marker filename for tracking installed system skills version.
const SYSTEM_SKILLS_MARKER_FILENAME: &str = ".codex-system-skills.marker";

/// Salt for fingerprint to invalidate on schema changes.
const SYSTEM_SKILLS_MARKER_SALT: &str = "mms-v1";

/// System skills directory name.
const SYSTEM_SKILLS_DIR_NAME: &str = ".system";

/// Skills directory name.
const SKILLS_DIR_NAME: &str = "skills";

/// Embedded system skill definition.
struct EmbeddedSkill {
    name: &'static str,
    content: &'static str,
}

/// All embedded system skills.
const EMBEDDED_SKILLS: &[EmbeddedSkill] = &[
    EmbeddedSkill {
        name: "help",
        content: builtin::HELP_SKILL,
    },
    EmbeddedSkill {
        name: "review",
        content: builtin::REVIEW_SKILL,
    },
    EmbeddedSkill {
        name: "compact",
        content: builtin::COMPACT_SKILL,
    },
    EmbeddedSkill {
        name: "undo",
        content: builtin::UNDO_SKILL,
    },
    EmbeddedSkill {
        name: "create-skill",
        content: builtin::CREATE_SKILL,
    },
];

/// Get the system skills cache root directory.
pub fn system_cache_root_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(SKILLS_DIR_NAME).join(SYSTEM_SKILLS_DIR_NAME)
}

/// Compute fingerprint of all embedded system skills.
///
/// The fingerprint is a hash of:
/// - Salt string (for version invalidation)
/// - Each skill name
/// - Each skill content hash
fn compute_embedded_fingerprint() -> String {
    let mut hasher = DefaultHasher::new();

    // Hash the salt first
    SYSTEM_SKILLS_MARKER_SALT.hash(&mut hasher);

    // Hash each skill's name and content
    for skill in EMBEDDED_SKILLS {
        skill.name.hash(&mut hasher);

        // Hash the content separately for better collision resistance
        let mut content_hasher = DefaultHasher::new();
        skill.content.hash(&mut content_hasher);
        content_hasher.finish().hash(&mut hasher);
    }

    format!("{:x}", hasher.finish())
}

/// Read the marker file to get the current fingerprint.
fn read_marker(marker_path: &Path) -> Option<String> {
    fs::read_to_string(marker_path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Check if system skills need to be reinstalled.
fn needs_reinstall(system_dir: &Path) -> bool {
    if !system_dir.exists() {
        debug!("System skills directory does not exist, needs install");
        return true;
    }

    let marker_path = system_dir.join(SYSTEM_SKILLS_MARKER_FILENAME);
    let expected_fingerprint = compute_embedded_fingerprint();

    match read_marker(&marker_path) {
        Some(current) if current == expected_fingerprint => {
            debug!("System skills fingerprint matches, skipping reinstall");
            false
        }
        Some(current) => {
            debug!(
                "System skills fingerprint mismatch: expected {}, found {}",
                expected_fingerprint, current
            );
            true
        }
        None => {
            debug!("No marker file found, needs install");
            true
        }
    }
}

/// Install embedded system skills to the user's skill directory.
///
/// Uses fingerprinting to avoid unnecessary reinstalls. The fingerprint
/// is computed from the embedded skill contents and stored in a marker file.
///
/// # Arguments
/// * `codex_home` - The user's codex home directory (typically ~/.codex)
///
/// # Returns
/// * `Ok(true)` - Skills were installed
/// * `Ok(false)` - Skills were already up to date (skipped)
/// * `Err(_)` - Installation failed
pub fn install_system_skills(codex_home: &Path) -> io::Result<bool> {
    let skills_root = codex_home.join(SKILLS_DIR_NAME);
    let system_dir = system_cache_root_dir(codex_home);

    // Check if we need to reinstall
    if !needs_reinstall(&system_dir) {
        return Ok(false);
    }

    info!("Installing system skills to {:?}", system_dir);

    // Clear existing system skills if present
    if system_dir.exists() {
        debug!("Removing existing system skills directory");
        fs::remove_dir_all(&system_dir)?;
    }

    // Create directories
    fs::create_dir_all(&skills_root)?;
    fs::create_dir_all(&system_dir)?;

    // Install each skill
    for skill in EMBEDDED_SKILLS {
        let skill_dir = system_dir.join(skill.name);
        fs::create_dir_all(&skill_dir)?;

        let skill_file = skill_dir.join("SKILL.md");
        fs::write(&skill_file, skill.content)?;

        debug!("Installed skill: {}", skill.name);
    }

    // Write the marker file with fingerprint
    let marker_path = system_dir.join(SYSTEM_SKILLS_MARKER_FILENAME);
    let fingerprint = compute_embedded_fingerprint();
    fs::write(&marker_path, format!("{}\n", fingerprint))?;

    info!(
        "Installed {} system skills with fingerprint {}",
        EMBEDDED_SKILLS.len(),
        fingerprint
    );

    Ok(true)
}

/// Uninstall system skills (for testing or cleanup).
pub fn uninstall_system_skills(codex_home: &Path) -> io::Result<()> {
    let system_dir = system_cache_root_dir(codex_home);

    if system_dir.exists() {
        fs::remove_dir_all(&system_dir)?;
        info!("Uninstalled system skills from {:?}", system_dir);
    }

    Ok(())
}

/// Get list of installed system skill names.
pub fn list_installed_skills(codex_home: &Path) -> io::Result<Vec<String>> {
    let system_dir = system_cache_root_dir(codex_home);

    if !system_dir.exists() {
        return Ok(vec![]);
    }

    let mut skills = Vec::new();

    for entry in fs::read_dir(&system_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Skip marker file
                if !name.starts_with('.') {
                    // Check if SKILL.md exists
                    if path.join("SKILL.md").exists() {
                        skills.push(name.to_string());
                    }
                }
            }
        }
    }

    skills.sort();
    Ok(skills)
}

/// Check if system skills are installed and up to date.
pub fn is_installed(codex_home: &Path) -> bool {
    let system_dir = system_cache_root_dir(codex_home);
    !needs_reinstall(&system_dir)
}

/// Get the current installed fingerprint, if any.
pub fn current_fingerprint(codex_home: &Path) -> Option<String> {
    let system_dir = system_cache_root_dir(codex_home);
    let marker_path = system_dir.join(SYSTEM_SKILLS_MARKER_FILENAME);
    read_marker(&marker_path)
}

/// Get the expected fingerprint for current embedded skills.
pub fn expected_fingerprint() -> String {
    compute_embedded_fingerprint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_fingerprint_is_stable() {
        let fp1 = compute_embedded_fingerprint();
        let fp2 = compute_embedded_fingerprint();
        assert_eq!(fp1, fp2, "Fingerprint should be stable");
    }

    #[test]
    fn test_install_creates_skills() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        let installed = install_system_skills(codex_home).unwrap();
        assert!(installed, "Should report installation");

        let skills = list_installed_skills(codex_home).unwrap();
        assert_eq!(skills.len(), EMBEDDED_SKILLS.len());
        assert!(skills.contains(&"help".to_string()));
        assert!(skills.contains(&"review".to_string()));
    }

    #[test]
    fn test_install_is_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        // First install
        let installed1 = install_system_skills(codex_home).unwrap();
        assert!(installed1, "First install should happen");

        // Second install should be skipped
        let installed2 = install_system_skills(codex_home).unwrap();
        assert!(!installed2, "Second install should be skipped");
    }

    #[test]
    fn test_marker_file_created() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        install_system_skills(codex_home).unwrap();

        let marker_path = system_cache_root_dir(codex_home)
            .join(SYSTEM_SKILLS_MARKER_FILENAME);
        assert!(marker_path.exists(), "Marker file should exist");

        let fp = current_fingerprint(codex_home);
        assert!(fp.is_some(), "Should read fingerprint");
        assert_eq!(fp.unwrap(), expected_fingerprint());
    }

    #[test]
    fn test_uninstall() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        install_system_skills(codex_home).unwrap();
        assert!(is_installed(codex_home));

        uninstall_system_skills(codex_home).unwrap();
        assert!(!is_installed(codex_home));
    }

    #[test]
    fn test_reinstall_on_changed_fingerprint() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        // Install
        install_system_skills(codex_home).unwrap();

        // Corrupt the marker file
        let marker_path = system_cache_root_dir(codex_home)
            .join(SYSTEM_SKILLS_MARKER_FILENAME);
        fs::write(&marker_path, "corrupted_fingerprint\n").unwrap();

        // Should reinstall
        let installed = install_system_skills(codex_home).unwrap();
        assert!(installed, "Should reinstall on fingerprint mismatch");
    }
}
