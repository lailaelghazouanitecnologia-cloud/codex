mod manager;
mod window;
mod truncation;
pub mod environment;

pub use manager::{ContextConfig, ContextManager};
pub use window::ContextWindow;
pub use truncation::{ContextMessage, TruncationStrategy};
pub use environment::{
    EnvironmentContext, EnvironmentContextBuilder, GitInfo, PlatformInfo,
    ShellInfo, ToolAvailability, ToolInfo, UserInfo,
};
