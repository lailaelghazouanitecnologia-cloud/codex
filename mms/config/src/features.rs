//! Feature flags for controlling agent capabilities.
//!
//! This module provides a comprehensive feature flag system for enabling/disabling
//! various agent capabilities at runtime.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Feature categories for grouping related features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureCategory {
    /// Tool-related features.
    Tools,
    /// User interface features.
    Ui,
    /// Security and policy features.
    Security,
    /// External integration features.
    Integration,
    /// Agent behavior features.
    Behavior,
    /// Model provider features.
    Provider,
    /// Development and debugging features.
    Development,
}

/// Individual feature flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    // ─── Tools ──────────────────────────────────────────────────────────────

    /// Shell/Bash command execution tool.
    ShellTool,
    /// File read tool.
    FileReadTool,
    /// File write tool.
    FileWriteTool,
    /// File edit tool.
    FileEditTool,
    /// Glob pattern file search.
    GlobTool,
    /// Grep content search.
    GrepTool,
    /// Web search tool.
    WebSearch,
    /// Web fetch tool.
    WebFetch,
    /// Jupyter notebook edit tool.
    NotebookEdit,
    /// Todo list management tool.
    TodoWrite,
    /// Agent/Task spawning tool.
    AgentTool,
    /// Plan mode tool.
    PlanMode,
    /// Multi-file edit tool.
    MultiEdit,
    /// Image/vision tool.
    ImageTool,
    /// PDF reading tool.
    PdfTool,

    // ─── UI ─────────────────────────────────────────────────────────────────

    /// Response streaming.
    Streaming,
    /// Progress notifications.
    Notifications,
    /// Progress indicators.
    ProgressIndicator,
    /// Syntax highlighting.
    SyntaxHighlight,
    /// Diff view.
    DiffView,
    /// Markdown rendering.
    MarkdownRender,
    /// Status line display.
    StatusLine,
    /// Interactive prompts.
    InteractivePrompts,

    // ─── Security ───────────────────────────────────────────────────────────

    /// Sandbox execution.
    Sandbox,
    /// Permission approval system.
    ApprovalSystem,
    /// Network access control.
    NetworkControl,
    /// File access control.
    FileAccessControl,
    /// Command filtering.
    CommandFiltering,
    /// Secret detection.
    SecretDetection,
    /// Audit logging.
    AuditLog,

    // ─── Integration ────────────────────────────────────────────────────────

    /// MCP server integration.
    McpIntegration,
    /// Git integration.
    GitIntegration,
    /// GitHub integration.
    GithubIntegration,
    /// Custom skills.
    Skills,
    /// Slash commands.
    SlashCommands,
    /// Hooks system.
    Hooks,
    /// Memory/context persistence.
    ContextPersistence,

    // ─── Behavior ───────────────────────────────────────────────────────────

    /// Parallel tool execution.
    ParallelTools,
    /// Context compaction/summarization.
    ContextCompaction,
    /// Automatic retry on failure.
    AutoRetry,
    /// Rate limit handling.
    RateLimitHandling,
    /// Session resumption.
    SessionResume,
    /// Conversation forking.
    ConversationFork,
    /// Extended thinking.
    ExtendedThinking,
    /// Reasoning output.
    ReasoningOutput,

    // ─── Provider ───────────────────────────────────────────────────────────

    /// Anthropic provider.
    ProviderAnthropic,
    /// OpenAI provider.
    ProviderOpenAI,
    /// Groq provider.
    ProviderGroq,
    /// Ollama provider.
    ProviderOllama,
    /// Gemini provider.
    ProviderGemini,
    /// Azure provider.
    ProviderAzure,
    /// AWS Bedrock provider.
    ProviderBedrock,
    /// Response caching.
    ResponseCaching,
    /// Token counting.
    TokenCounting,

    // ─── Development ────────────────────────────────────────────────────────

    /// Debug logging.
    DebugLogging,
    /// Performance tracing.
    PerformanceTracing,
    /// Dry run mode.
    DryRun,
    /// Request/response logging.
    RequestLogging,
    /// Development mode.
    DevMode,
    /// Test mode.
    TestMode,
}

impl Feature {
    /// Get the feature key for configuration files.
    pub fn key(&self) -> &'static str {
        match self {
            // Tools
            Self::ShellTool => "shell_tool",
            Self::FileReadTool => "file_read_tool",
            Self::FileWriteTool => "file_write_tool",
            Self::FileEditTool => "file_edit_tool",
            Self::GlobTool => "glob_tool",
            Self::GrepTool => "grep_tool",
            Self::WebSearch => "web_search",
            Self::WebFetch => "web_fetch",
            Self::NotebookEdit => "notebook_edit",
            Self::TodoWrite => "todo_write",
            Self::AgentTool => "agent_tool",
            Self::PlanMode => "plan_mode",
            Self::MultiEdit => "multi_edit",
            Self::ImageTool => "image_tool",
            Self::PdfTool => "pdf_tool",
            // UI
            Self::Streaming => "streaming",
            Self::Notifications => "notifications",
            Self::ProgressIndicator => "progress_indicator",
            Self::SyntaxHighlight => "syntax_highlight",
            Self::DiffView => "diff_view",
            Self::MarkdownRender => "markdown_render",
            Self::StatusLine => "status_line",
            Self::InteractivePrompts => "interactive_prompts",
            // Security
            Self::Sandbox => "sandbox",
            Self::ApprovalSystem => "approval_system",
            Self::NetworkControl => "network_control",
            Self::FileAccessControl => "file_access_control",
            Self::CommandFiltering => "command_filtering",
            Self::SecretDetection => "secret_detection",
            Self::AuditLog => "audit_log",
            // Integration
            Self::McpIntegration => "mcp_integration",
            Self::GitIntegration => "git_integration",
            Self::GithubIntegration => "github_integration",
            Self::Skills => "skills",
            Self::SlashCommands => "slash_commands",
            Self::Hooks => "hooks",
            Self::ContextPersistence => "context_persistence",
            // Behavior
            Self::ParallelTools => "parallel_tools",
            Self::ContextCompaction => "context_compaction",
            Self::AutoRetry => "auto_retry",
            Self::RateLimitHandling => "rate_limit_handling",
            Self::SessionResume => "session_resume",
            Self::ConversationFork => "conversation_fork",
            Self::ExtendedThinking => "extended_thinking",
            Self::ReasoningOutput => "reasoning_output",
            // Provider
            Self::ProviderAnthropic => "provider_anthropic",
            Self::ProviderOpenAI => "provider_openai",
            Self::ProviderGroq => "provider_groq",
            Self::ProviderOllama => "provider_ollama",
            Self::ProviderGemini => "provider_gemini",
            Self::ProviderAzure => "provider_azure",
            Self::ProviderBedrock => "provider_bedrock",
            Self::ResponseCaching => "response_caching",
            Self::TokenCounting => "token_counting",
            // Development
            Self::DebugLogging => "debug_logging",
            Self::PerformanceTracing => "performance_tracing",
            Self::DryRun => "dry_run",
            Self::RequestLogging => "request_logging",
            Self::DevMode => "dev_mode",
            Self::TestMode => "test_mode",
        }
    }

    /// Get the feature's display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            // Tools
            Self::ShellTool => "Shell Tool",
            Self::FileReadTool => "File Read",
            Self::FileWriteTool => "File Write",
            Self::FileEditTool => "File Edit",
            Self::GlobTool => "Glob Search",
            Self::GrepTool => "Grep Search",
            Self::WebSearch => "Web Search",
            Self::WebFetch => "Web Fetch",
            Self::NotebookEdit => "Notebook Edit",
            Self::TodoWrite => "Todo List",
            Self::AgentTool => "Agent Tool",
            Self::PlanMode => "Plan Mode",
            Self::MultiEdit => "Multi-Edit",
            Self::ImageTool => "Image Tool",
            Self::PdfTool => "PDF Tool",
            // UI
            Self::Streaming => "Streaming",
            Self::Notifications => "Notifications",
            Self::ProgressIndicator => "Progress Indicator",
            Self::SyntaxHighlight => "Syntax Highlighting",
            Self::DiffView => "Diff View",
            Self::MarkdownRender => "Markdown Render",
            Self::StatusLine => "Status Line",
            Self::InteractivePrompts => "Interactive Prompts",
            // Security
            Self::Sandbox => "Sandbox",
            Self::ApprovalSystem => "Approval System",
            Self::NetworkControl => "Network Control",
            Self::FileAccessControl => "File Access Control",
            Self::CommandFiltering => "Command Filtering",
            Self::SecretDetection => "Secret Detection",
            Self::AuditLog => "Audit Log",
            // Integration
            Self::McpIntegration => "MCP Integration",
            Self::GitIntegration => "Git Integration",
            Self::GithubIntegration => "GitHub Integration",
            Self::Skills => "Skills",
            Self::SlashCommands => "Slash Commands",
            Self::Hooks => "Hooks",
            Self::ContextPersistence => "Context Persistence",
            // Behavior
            Self::ParallelTools => "Parallel Tools",
            Self::ContextCompaction => "Context Compaction",
            Self::AutoRetry => "Auto Retry",
            Self::RateLimitHandling => "Rate Limit Handling",
            Self::SessionResume => "Session Resume",
            Self::ConversationFork => "Conversation Fork",
            Self::ExtendedThinking => "Extended Thinking",
            Self::ReasoningOutput => "Reasoning Output",
            // Provider
            Self::ProviderAnthropic => "Anthropic",
            Self::ProviderOpenAI => "OpenAI",
            Self::ProviderGroq => "Groq",
            Self::ProviderOllama => "Ollama",
            Self::ProviderGemini => "Gemini",
            Self::ProviderAzure => "Azure",
            Self::ProviderBedrock => "AWS Bedrock",
            Self::ResponseCaching => "Response Caching",
            Self::TokenCounting => "Token Counting",
            // Development
            Self::DebugLogging => "Debug Logging",
            Self::PerformanceTracing => "Performance Tracing",
            Self::DryRun => "Dry Run",
            Self::RequestLogging => "Request Logging",
            Self::DevMode => "Dev Mode",
            Self::TestMode => "Test Mode",
        }
    }

    /// Get the feature's category.
    pub fn category(&self) -> FeatureCategory {
        match self {
            Self::ShellTool
            | Self::FileReadTool
            | Self::FileWriteTool
            | Self::FileEditTool
            | Self::GlobTool
            | Self::GrepTool
            | Self::WebSearch
            | Self::WebFetch
            | Self::NotebookEdit
            | Self::TodoWrite
            | Self::AgentTool
            | Self::PlanMode
            | Self::MultiEdit
            | Self::ImageTool
            | Self::PdfTool => FeatureCategory::Tools,

            Self::Streaming
            | Self::Notifications
            | Self::ProgressIndicator
            | Self::SyntaxHighlight
            | Self::DiffView
            | Self::MarkdownRender
            | Self::StatusLine
            | Self::InteractivePrompts => FeatureCategory::Ui,

            Self::Sandbox
            | Self::ApprovalSystem
            | Self::NetworkControl
            | Self::FileAccessControl
            | Self::CommandFiltering
            | Self::SecretDetection
            | Self::AuditLog => FeatureCategory::Security,

            Self::McpIntegration
            | Self::GitIntegration
            | Self::GithubIntegration
            | Self::Skills
            | Self::SlashCommands
            | Self::Hooks
            | Self::ContextPersistence => FeatureCategory::Integration,

            Self::ParallelTools
            | Self::ContextCompaction
            | Self::AutoRetry
            | Self::RateLimitHandling
            | Self::SessionResume
            | Self::ConversationFork
            | Self::ExtendedThinking
            | Self::ReasoningOutput => FeatureCategory::Behavior,

            Self::ProviderAnthropic
            | Self::ProviderOpenAI
            | Self::ProviderGroq
            | Self::ProviderOllama
            | Self::ProviderGemini
            | Self::ProviderAzure
            | Self::ProviderBedrock
            | Self::ResponseCaching
            | Self::TokenCounting => FeatureCategory::Provider,

            Self::DebugLogging
            | Self::PerformanceTracing
            | Self::DryRun
            | Self::RequestLogging
            | Self::DevMode
            | Self::TestMode => FeatureCategory::Development,
        }
    }

    /// Check if this feature is enabled by default.
    pub fn default_enabled(&self) -> bool {
        match self {
            // Default enabled tools
            Self::ShellTool
            | Self::FileReadTool
            | Self::FileWriteTool
            | Self::FileEditTool
            | Self::GlobTool
            | Self::GrepTool
            | Self::TodoWrite
            | Self::PlanMode => true,

            // Default enabled UI
            Self::Streaming
            | Self::ProgressIndicator
            | Self::SyntaxHighlight
            | Self::DiffView
            | Self::MarkdownRender
            | Self::StatusLine
            | Self::InteractivePrompts => true,

            // Default enabled security
            Self::Sandbox
            | Self::ApprovalSystem
            | Self::CommandFiltering
            | Self::SecretDetection => true,

            // Default enabled integration
            Self::GitIntegration
            | Self::Skills
            | Self::SlashCommands
            | Self::Hooks
            | Self::ContextPersistence => true,

            // Default enabled behavior
            Self::ParallelTools
            | Self::ContextCompaction
            | Self::AutoRetry
            | Self::RateLimitHandling
            | Self::SessionResume => true,

            // Default enabled providers
            Self::ProviderAnthropic | Self::TokenCounting => true,

            // Default disabled
            _ => false,
        }
    }

    /// Get the feature's description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::ShellTool => "Execute shell commands",
            Self::FileReadTool => "Read file contents",
            Self::FileWriteTool => "Write files",
            Self::FileEditTool => "Edit existing files",
            Self::GlobTool => "Search files by pattern",
            Self::GrepTool => "Search file contents",
            Self::WebSearch => "Search the web",
            Self::WebFetch => "Fetch web pages",
            Self::NotebookEdit => "Edit Jupyter notebooks",
            Self::TodoWrite => "Manage task lists",
            Self::AgentTool => "Spawn sub-agents",
            Self::PlanMode => "Plan before implementing",
            Self::MultiEdit => "Edit multiple files atomically",
            Self::ImageTool => "Analyze images",
            Self::PdfTool => "Read PDF documents",
            Self::Streaming => "Stream responses in real-time",
            Self::Notifications => "Show progress notifications",
            Self::ProgressIndicator => "Display progress bars",
            Self::SyntaxHighlight => "Highlight code syntax",
            Self::DiffView => "Show file differences",
            Self::MarkdownRender => "Render markdown content",
            Self::StatusLine => "Show status bar",
            Self::InteractivePrompts => "Interactive user prompts",
            Self::Sandbox => "Run commands in sandbox",
            Self::ApprovalSystem => "Require user approval",
            Self::NetworkControl => "Control network access",
            Self::FileAccessControl => "Control file access",
            Self::CommandFiltering => "Filter dangerous commands",
            Self::SecretDetection => "Detect secrets in code",
            Self::AuditLog => "Log all operations",
            Self::McpIntegration => "Connect to MCP servers",
            Self::GitIntegration => "Git repository support",
            Self::GithubIntegration => "GitHub API integration",
            Self::Skills => "Load custom skills",
            Self::SlashCommands => "Custom slash commands",
            Self::Hooks => "Lifecycle hooks",
            Self::ContextPersistence => "Persist conversation context",
            Self::ParallelTools => "Run tools in parallel",
            Self::ContextCompaction => "Summarize long contexts",
            Self::AutoRetry => "Retry failed operations",
            Self::RateLimitHandling => "Handle rate limits",
            Self::SessionResume => "Resume previous sessions",
            Self::ConversationFork => "Fork conversations",
            Self::ExtendedThinking => "Extended reasoning time",
            Self::ReasoningOutput => "Show reasoning process",
            Self::ProviderAnthropic => "Use Anthropic models",
            Self::ProviderOpenAI => "Use OpenAI models",
            Self::ProviderGroq => "Use Groq models",
            Self::ProviderOllama => "Use Ollama models",
            Self::ProviderGemini => "Use Gemini models",
            Self::ProviderAzure => "Use Azure OpenAI",
            Self::ProviderBedrock => "Use AWS Bedrock",
            Self::ResponseCaching => "Cache API responses",
            Self::TokenCounting => "Count tokens",
            Self::DebugLogging => "Enable debug logs",
            Self::PerformanceTracing => "Trace performance",
            Self::DryRun => "Simulate without executing",
            Self::RequestLogging => "Log API requests",
            Self::DevMode => "Development mode",
            Self::TestMode => "Testing mode",
        }
    }

    /// Parse a feature from its key.
    pub fn from_key(key: &str) -> Option<Self> {
        ALL_FEATURES.iter().find(|f| f.key() == key).copied()
    }
}

/// All available features.
pub static ALL_FEATURES: &[Feature] = &[
    // Tools
    Feature::ShellTool,
    Feature::FileReadTool,
    Feature::FileWriteTool,
    Feature::FileEditTool,
    Feature::GlobTool,
    Feature::GrepTool,
    Feature::WebSearch,
    Feature::WebFetch,
    Feature::NotebookEdit,
    Feature::TodoWrite,
    Feature::AgentTool,
    Feature::PlanMode,
    Feature::MultiEdit,
    Feature::ImageTool,
    Feature::PdfTool,
    // UI
    Feature::Streaming,
    Feature::Notifications,
    Feature::ProgressIndicator,
    Feature::SyntaxHighlight,
    Feature::DiffView,
    Feature::MarkdownRender,
    Feature::StatusLine,
    Feature::InteractivePrompts,
    // Security
    Feature::Sandbox,
    Feature::ApprovalSystem,
    Feature::NetworkControl,
    Feature::FileAccessControl,
    Feature::CommandFiltering,
    Feature::SecretDetection,
    Feature::AuditLog,
    // Integration
    Feature::McpIntegration,
    Feature::GitIntegration,
    Feature::GithubIntegration,
    Feature::Skills,
    Feature::SlashCommands,
    Feature::Hooks,
    Feature::ContextPersistence,
    // Behavior
    Feature::ParallelTools,
    Feature::ContextCompaction,
    Feature::AutoRetry,
    Feature::RateLimitHandling,
    Feature::SessionResume,
    Feature::ConversationFork,
    Feature::ExtendedThinking,
    Feature::ReasoningOutput,
    // Provider
    Feature::ProviderAnthropic,
    Feature::ProviderOpenAI,
    Feature::ProviderGroq,
    Feature::ProviderOllama,
    Feature::ProviderGemini,
    Feature::ProviderAzure,
    Feature::ProviderBedrock,
    Feature::ResponseCaching,
    Feature::TokenCounting,
    // Development
    Feature::DebugLogging,
    Feature::PerformanceTracing,
    Feature::DryRun,
    Feature::RequestLogging,
    Feature::DevMode,
    Feature::TestMode,
];

/// Feature configuration and state management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    /// Enabled features.
    enabled: BTreeSet<Feature>,

    /// Feature-specific configuration overrides.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    overrides: BTreeMap<Feature, FeatureConfig>,
}

/// Configuration for a specific feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Whether the feature is enabled.
    pub enabled: bool,

    /// Custom options for the feature.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, serde_json::Value>,
}

impl Features {
    /// Create a new Features instance with no features enabled.
    pub fn new() -> Self {
        Self {
            enabled: BTreeSet::new(),
            overrides: BTreeMap::new(),
        }
    }

    /// Create a Features instance with default features enabled.
    pub fn with_defaults() -> Self {
        let enabled = ALL_FEATURES
            .iter()
            .filter(|f| f.default_enabled())
            .copied()
            .collect();

        Self {
            enabled,
            overrides: BTreeMap::new(),
        }
    }

    /// Create a minimal feature set (only essential features).
    pub fn minimal() -> Self {
        let mut features = Self::new();
        features.enable(Feature::FileReadTool);
        features.enable(Feature::Streaming);
        features.enable(Feature::ProviderAnthropic);
        features
    }

    /// Create a full feature set (all features enabled).
    pub fn full() -> Self {
        Self {
            enabled: ALL_FEATURES.iter().copied().collect(),
            overrides: BTreeMap::new(),
        }
    }

    /// Get all available features.
    pub fn all_features() -> &'static [Feature] {
        ALL_FEATURES
    }

    /// Check if a feature is enabled.
    pub fn enabled(&self, feature: Feature) -> bool {
        if let Some(config) = self.overrides.get(&feature) {
            return config.enabled;
        }
        self.enabled.contains(&feature)
    }

    /// Enable a feature.
    pub fn enable(&mut self, feature: Feature) -> &mut Self {
        self.enabled.insert(feature);
        self.overrides.remove(&feature);
        self
    }

    /// Disable a feature.
    pub fn disable(&mut self, feature: Feature) -> &mut Self {
        self.enabled.remove(&feature);
        self.overrides.remove(&feature);
        self
    }

    /// Set a feature's enabled state.
    pub fn set(&mut self, feature: Feature, enabled: bool) -> &mut Self {
        if enabled {
            self.enable(feature)
        } else {
            self.disable(feature)
        }
    }

    /// Set feature configuration.
    pub fn set_config(&mut self, feature: Feature, config: FeatureConfig) -> &mut Self {
        self.overrides.insert(feature, config);
        self
    }

    /// Get feature configuration.
    pub fn get_config(&self, feature: Feature) -> Option<&FeatureConfig> {
        self.overrides.get(&feature)
    }

    /// Get an iterator over enabled features.
    pub fn enabled_features(&self) -> impl Iterator<Item = Feature> + '_ {
        self.enabled.iter().copied()
    }

    /// Get features by category.
    pub fn features_by_category(&self, category: FeatureCategory) -> Vec<Feature> {
        ALL_FEATURES
            .iter()
            .filter(|f| f.category() == category)
            .copied()
            .collect()
    }

    /// Get enabled features by category.
    pub fn enabled_by_category(&self, category: FeatureCategory) -> Vec<Feature> {
        self.enabled
            .iter()
            .filter(|f| f.category() == category)
            .copied()
            .collect()
    }

    /// Count enabled features.
    pub fn enabled_count(&self) -> usize {
        self.enabled.len()
    }

    /// Check if any tool features are enabled.
    pub fn has_tools(&self) -> bool {
        self.enabled
            .iter()
            .any(|f| f.category() == FeatureCategory::Tools)
    }

    /// Check if any provider is enabled.
    pub fn has_provider(&self) -> bool {
        self.enabled(Feature::ProviderAnthropic)
            || self.enabled(Feature::ProviderOpenAI)
            || self.enabled(Feature::ProviderGroq)
            || self.enabled(Feature::ProviderOllama)
            || self.enabled(Feature::ProviderGemini)
            || self.enabled(Feature::ProviderAzure)
            || self.enabled(Feature::ProviderBedrock)
    }

    /// Enable all features in a category.
    pub fn enable_category(&mut self, category: FeatureCategory) -> &mut Self {
        for feature in ALL_FEATURES.iter().filter(|f| f.category() == category) {
            self.enable(*feature);
        }
        self
    }

    /// Disable all features in a category.
    pub fn disable_category(&mut self, category: FeatureCategory) -> &mut Self {
        for feature in ALL_FEATURES.iter().filter(|f| f.category() == category) {
            self.disable(*feature);
        }
        self
    }

    /// Merge features from another instance (enabled features are added).
    pub fn merge(&mut self, other: &Features) -> &mut Self {
        self.enabled.extend(other.enabled.iter().copied());
        for (feature, config) in &other.overrides {
            self.overrides.insert(*feature, config.clone());
        }
        self
    }

    /// Create a summary of enabled features.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        for category in [
            FeatureCategory::Tools,
            FeatureCategory::Ui,
            FeatureCategory::Security,
            FeatureCategory::Integration,
            FeatureCategory::Behavior,
            FeatureCategory::Provider,
            FeatureCategory::Development,
        ] {
            let enabled = self.enabled_by_category(category);
            if !enabled.is_empty() {
                let names: Vec<_> = enabled.iter().map(|f| f.display_name()).collect();
                parts.push(format!("{:?}: {}", category, names.join(", ")));
            }
        }

        parts.join("\n")
    }
}

impl Default for Features {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_features() {
        let features = Features::with_defaults();
        assert!(features.enabled(Feature::ShellTool));
        assert!(features.enabled(Feature::FileReadTool));
        assert!(features.enabled(Feature::Streaming));
        assert!(!features.enabled(Feature::WebSearch));
    }

    #[test]
    fn test_enable_disable() {
        let mut features = Features::new();
        assert!(!features.enabled(Feature::WebSearch));

        features.enable(Feature::WebSearch);
        assert!(features.enabled(Feature::WebSearch));

        features.disable(Feature::WebSearch);
        assert!(!features.enabled(Feature::WebSearch));
    }

    #[test]
    fn test_category() {
        assert_eq!(Feature::ShellTool.category(), FeatureCategory::Tools);
        assert_eq!(Feature::Streaming.category(), FeatureCategory::Ui);
        assert_eq!(Feature::Sandbox.category(), FeatureCategory::Security);
    }

    #[test]
    fn test_from_key() {
        assert_eq!(Feature::from_key("shell_tool"), Some(Feature::ShellTool));
        assert_eq!(Feature::from_key("unknown"), None);
    }
}
