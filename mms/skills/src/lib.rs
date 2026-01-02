//! Skills system for context-aware task instructions.
//!
//! This crate provides a comprehensive skills system that enables:
//! - Loading skills from markdown files with YAML frontmatter
//! - Skill discovery from multiple scopes (repo, user, system, admin)
//! - Built-in skills for common tasks (help, review, compact, undo)
//! - Skill injection into conversation context
//! - System prompt generation with available skills
//!
//! # Overview
//!
//! Skills are custom instructions loaded from `SKILL.md` files that guide
//! the assistant for specific tasks. They provide a way to customize
//! behavior without modifying the core system.
//!
//! # Example
//!
//! ```ignore
//! use mms_skills::{SkillManager, SkillInstructions};
//! use std::path::Path;
//!
//! // Create and initialize the skill manager
//! let mut manager = SkillManager::new();
//! manager.initialize().unwrap();
//!
//! // Load skills for a working directory
//! let cwd = Path::new("/path/to/project");
//! let outcome = manager.skills_for_cwd(cwd);
//!
//! println!("Found {} skills", outcome.skill_count());
//!
//! // Get a specific skill
//! if let Some(skill) = manager.get_skill(cwd, "review") {
//!     // Load full instructions for injection
//!     let instructions = SkillInstructions::from_metadata(&skill).unwrap();
//!     println!("Skill: {}", instructions.to_injection_format());
//! }
//!
//! // Generate skills section for system prompt
//! let skills_section = mms_skills::render::render_skills_section(&outcome.skills);
//! ```
//!
//! # Skill Structure
//!
//! A skill is a directory containing:
//!
//! ```text
//! my-skill/
//! ├── SKILL.md       # Required: Skill definition with YAML frontmatter
//! ├── scripts/       # Optional: Executable scripts
//! ├── references/    # Optional: Documentation files
//! └── assets/        # Optional: Templates, examples
//! ```
//!
//! # SKILL.md Format
//!
//! ```markdown
//! ---
//! name: my-skill
//! description: When to use this skill
//! metadata:
//!   short-description: Brief label
//!   version: "1.0"
//!   tags:
//!     - category
//! ---
//! # Instructions
//!
//! Detailed instructions for the assistant...
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

pub mod builtin;
pub mod loader;
pub mod manager;
pub mod model;
pub mod registry;
pub mod render;
mod skill;
pub mod system;

// Re-export main types
pub use loader::{SkillError, SkillLoadOutcome, SkillLoader};
pub use manager::{SharedSkillManager, SkillManager, SystemSkillsInfo, new_shared_manager};
pub use model::{SkillInstructions, SkillMetadata, SkillScope, SkillsSummary};
pub use registry::SkillRegistry;
pub use skill::{Skill, SkillContext, SkillOutput, SkillSpec};

// Re-export system skill functions
pub use system::{
    install_system_skills, uninstall_system_skills, list_installed_skills,
    is_installed as system_skills_installed, current_fingerprint, expected_fingerprint,
    system_cache_root_dir,
};

/// Prelude for commonly used types.
pub mod prelude {
    pub use crate::{
        SkillContext, SkillInstructions, SkillLoader, SkillManager,
        SkillMetadata, SkillOutput, SkillRegistry, SkillScope,
    };
}
