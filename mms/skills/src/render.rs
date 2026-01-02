//! Rendering skills section for system prompts.

use crate::model::SkillMetadata;

/// Render a skills section for inclusion in system prompts.
///
/// This creates a markdown section listing all available skills with
/// their descriptions and trigger information.
pub fn render_skills_section(skills: &[SkillMetadata]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut output = String::new();

    output.push_str("# Skills\n\n");
    output.push_str("The following skills are available to help with specific tasks. ");
    output.push_str("When a user's request matches a skill's description, ");
    output.push_str("you should apply that skill's instructions.\n\n");

    output.push_str("## Available Skills\n\n");

    for skill in skills {
        output.push_str(&format!("### {}\n", skill.name));

        // Description
        output.push_str(&format!("**When to use**: {}\n", skill.description));

        // Scope indicator
        output.push_str(&format!("**Scope**: {}\n", skill.scope));

        // Tags if available
        if !skill.tags.is_empty() {
            output.push_str(&format!("**Tags**: {}\n", skill.tags.join(", ")));
        }

        output.push('\n');
    }

    output.push_str("## Skill Usage Rules\n\n");
    output.push_str("1. **Explicit Mention**: Use a skill when the user explicitly mentions it by name\n");
    output.push_str("2. **Description Match**: Use a skill when the task clearly matches its description\n");
    output.push_str("3. **Multiple Skills**: If multiple skills are mentioned, apply all of them\n");
    output.push_str("4. **Turn Scope**: Skills apply to the current turn unless re-mentioned\n");
    output.push_str("5. **Progressive Loading**: Skill details are loaded on-demand when activated\n\n");

    output.push_str("## Creating Custom Skills\n\n");
    output.push_str("Users can create custom skills by adding `SKILL.md` files to:\n");
    output.push_str("- `.codex/skills/` - Project-specific skills\n");
    output.push_str("- `~/.codex/skills/` - User-wide skills\n\n");
    output.push_str("Use the `create-skill` skill for guidance on creating new skills.\n");

    output
}

/// Render a compact skill list for status display.
pub fn render_skill_list(skills: &[SkillMetadata]) -> String {
    if skills.is_empty() {
        return "No skills available".to_string();
    }

    skills
        .iter()
        .map(|s| {
            let desc = s.short_description.as_deref().unwrap_or(&s.description);
            format!("- **{}**: {}", s.name, desc)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render skill metadata as a table.
pub fn render_skill_table(skills: &[SkillMetadata]) -> String {
    if skills.is_empty() {
        return "No skills available".to_string();
    }

    let mut output = String::new();

    output.push_str("| Name | Description | Scope |\n");
    output.push_str("|------|-------------|-------|\n");

    for skill in skills {
        let desc = skill.short_description.as_deref().unwrap_or(&skill.description);
        // Truncate long descriptions
        let desc = if desc.len() > 50 {
            format!("{}...", &desc[..47])
        } else {
            desc.to_string()
        };

        output.push_str(&format!(
            "| {} | {} | {} |\n",
            skill.name, desc, skill.scope
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SkillScope;
    use std::path::PathBuf;

    fn create_test_skill(name: &str, description: &str, scope: SkillScope) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: description.to_string(),
            short_description: None,
            path: PathBuf::from("/test/skill"),
            scope,
            version: None,
            author: None,
            tags: vec![],
            body: String::new(),
        }
    }

    #[test]
    fn test_render_skills_section() {
        let skills = vec![
            create_test_skill("review", "Review code changes", SkillScope::System),
            create_test_skill("help", "Get help information", SkillScope::System),
        ];

        let output = render_skills_section(&skills);

        assert!(output.contains("# Skills"));
        assert!(output.contains("### review"));
        assert!(output.contains("### help"));
        assert!(output.contains("Review code changes"));
        assert!(output.contains("Skill Usage Rules"));
    }

    #[test]
    fn test_render_empty_skills() {
        let skills: Vec<SkillMetadata> = vec![];
        let output = render_skills_section(&skills);
        assert!(output.is_empty());
    }

    #[test]
    fn test_render_skill_list() {
        let skills = vec![
            create_test_skill("review", "Review code changes", SkillScope::System),
        ];

        let output = render_skill_list(&skills);
        assert!(output.contains("**review**"));
        assert!(output.contains("Review code changes"));
    }

    #[test]
    fn test_render_skill_table() {
        let skills = vec![
            create_test_skill("review", "Review code changes", SkillScope::System),
        ];

        let output = render_skill_table(&skills);
        assert!(output.contains("| Name |"));
        assert!(output.contains("| review |"));
    }
}
