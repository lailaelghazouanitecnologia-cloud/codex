//! Built-in skill definitions.
//!
//! These skills are embedded in the binary and installed to the user's
//! `.codex/skills/.system/` directory on first run.

/// Help skill - provides information about available commands and features.
pub const HELP_SKILL: &str = r#"---
name: help
description: Get help with using the assistant, available commands, and features
metadata:
  short-description: Show help and usage information
  version: "1.0"
  tags:
    - builtin
    - help
    - documentation
---
# Help Skill

When the user asks for help, provide information about:

## Available Commands

- `/help` - Show this help information
- `/review` - Review code changes and provide feedback
- `/compact` - Summarize the conversation to reduce context size
- `/undo` - Revert changes made in the last turn

## Tool Usage

The assistant can use various tools to help with tasks:

### File Operations
- **read_file** - Read file contents
- **write_file** - Create or overwrite files
- **edit_file** - Make targeted edits to files
- **list_dir** - List directory contents
- **glob** - Find files matching patterns
- **grep** - Search file contents

### Shell Operations
- **shell** - Execute shell commands

## Skills

Skills are custom instructions that can be activated to guide the assistant
for specific tasks. Users can create their own skills by adding SKILL.md
files to:

- `.codex/skills/` - Project-specific skills
- `~/.codex/skills/` - User-wide skills

## Tips

1. Be specific about what you want to accomplish
2. Provide context about your project and constraints
3. Review changes before accepting them
4. Use `/undo` if something goes wrong
5. Break complex tasks into smaller steps
"#;

/// Review skill - code review assistance.
pub const REVIEW_SKILL: &str = r#"---
name: review
description: Review code changes, identify issues, and provide improvement suggestions
metadata:
  short-description: Code review assistant
  version: "1.0"
  tags:
    - builtin
    - code-review
    - quality
---
# Code Review Skill

When asked to review code, follow this structured approach:

## Review Process

1. **Understand the Context**
   - What is the purpose of these changes?
   - What problem is being solved?
   - Are there related files or tests?

2. **Check Code Quality**
   - Code readability and clarity
   - Consistent naming conventions
   - Appropriate comments and documentation
   - DRY principle (Don't Repeat Yourself)
   - Single Responsibility Principle

3. **Identify Potential Issues**
   - Logic errors or edge cases
   - Performance concerns
   - Security vulnerabilities
   - Error handling gaps
   - Resource leaks

4. **Verify Best Practices**
   - Language-specific idioms
   - Framework conventions
   - Project style guidelines
   - Test coverage

5. **Provide Actionable Feedback**
   - Be specific about what should change
   - Explain why changes are needed
   - Suggest concrete improvements
   - Prioritize critical vs. optional changes

## Output Format

Structure your review as:

```markdown
## Summary
Brief overview of the changes and overall assessment.

## Critical Issues
Must-fix problems that would cause bugs or security issues.

## Suggestions
Recommended improvements for code quality.

## Positive Highlights
Good practices or patterns worth acknowledging.
```

## Guidelines

- Be constructive and respectful
- Focus on the code, not the person
- Explain reasoning, not just opinions
- Consider the project context
- Acknowledge good work alongside issues
"#;

/// Compact skill - conversation summarization.
pub const COMPACT_SKILL: &str = r#"---
name: compact
description: Summarize the conversation to reduce context size while preserving important information
metadata:
  short-description: Summarize conversation context
  version: "1.0"
  tags:
    - builtin
    - context
    - summarization
---
# Compact Skill

When asked to compact the conversation, create a concise summary that:

## Preservation Priorities

1. **Critical Information**
   - Current task or goal
   - Important decisions made
   - Files that were modified
   - Pending items or blockers

2. **Context**
   - Project structure understanding
   - Technical constraints
   - User preferences observed

3. **State**
   - What has been completed
   - What is in progress
   - What needs to happen next

## Summary Format

```markdown
## Task Summary
[One-line description of the current goal]

## Completed Actions
- [Action 1]
- [Action 2]

## Modified Files
- path/to/file1.rs - [brief description of changes]
- path/to/file2.rs - [brief description of changes]

## Pending Work
- [ ] [Next step 1]
- [ ] [Next step 2]

## Key Decisions
- [Important decision or constraint]

## Technical Context
[Brief note about project structure or constraints]
```

## Guidelines

- Be concise but complete
- Prioritize actionable information
- Drop redundant or superseded details
- Keep file paths and code references accurate
- Preserve any user preferences or constraints
"#;

/// Undo skill - revert changes.
pub const UNDO_SKILL: &str = r#"---
name: undo
description: Revert file changes made in the last turn
metadata:
  short-description: Undo recent changes
  version: "1.0"
  tags:
    - builtin
    - undo
    - revert
---
# Undo Skill

When the user wants to undo changes, help them revert modifications:

## Undo Process

1. **Identify Changes**
   - List files that were modified in the last turn
   - Show what type of change was made (created, modified, deleted)
   - Display original content if available

2. **Confirm Scope**
   - Ask if user wants to undo all changes or specific files
   - Clarify if they want to undo just the last turn or more

3. **Execute Revert**
   - For created files: Delete them
   - For modified files: Restore original content
   - For deleted files: Recreate with original content
   - For renamed files: Rename back to original path

4. **Verify**
   - Confirm which files were reverted
   - Show the state after revert
   - Note any files that couldn't be reverted

## Safety Guidelines

- Always confirm before reverting
- Show what will be changed
- Handle partial reverts gracefully
- Preserve files not in the change set
- Report any errors clearly

## Limitations

- Can only undo changes from the current session
- Cannot undo changes already pushed to git
- External file modifications may not be tracked
- Binary files may have limited support
"#;

/// Create skill - meta-skill for creating new skills.
pub const CREATE_SKILL: &str = r#"---
name: create-skill
description: Guide for creating new custom skills with proper structure and documentation
metadata:
  short-description: Create custom skills
  version: "1.0"
  tags:
    - builtin
    - meta
    - skill-creation
---
# Create Skill Guide

Help the user create effective custom skills:

## Skill Structure

A skill is a directory containing:

```
my-skill/
├── SKILL.md       # Required: Skill definition
├── scripts/       # Optional: Executable scripts
├── references/    # Optional: Documentation files
└── assets/        # Optional: Templates, examples
```

## SKILL.md Format

```markdown
---
name: my-skill
description: Clear description of when to use this skill
metadata:
  short-description: Brief UI label
  version: "1.0"
  author: Your Name
  tags:
    - category1
    - category2
---
# Skill Name

## When to Use
Describe the situations where this skill applies.

## Instructions
Detailed instructions for the assistant to follow.

## Output Format
Expected format for results (if applicable).

## Examples
Concrete examples of usage (optional but helpful).

## Guidelines
Any constraints or best practices to follow.
```

## Best Practices

1. **Clear Trigger Conditions**
   - Write a description that clearly indicates when the skill applies
   - Use specific keywords that users might mention

2. **Actionable Instructions**
   - Provide step-by-step guidance
   - Include examples where helpful
   - Define expected output format

3. **Progressive Disclosure**
   - Keep the SKILL.md focused
   - Put supporting docs in references/
   - Put scripts for automation in scripts/

4. **Versioning**
   - Use semantic versioning
   - Update version when making changes
   - Keep changelog for complex skills

## Skill Locations

- **Project Skills**: `.codex/skills/` (for project-specific skills)
- **User Skills**: `~/.codex/skills/` (for personal skills)

## Tips

- Test your skill with various prompts
- Keep instructions concise but complete
- Use examples to clarify expectations
- Reference external docs rather than duplicating
"#;
