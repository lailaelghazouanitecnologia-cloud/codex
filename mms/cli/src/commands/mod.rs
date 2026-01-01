mod cli_mode;
mod config;
mod mcp;
mod web_mode;

pub use cli_mode::run_cli;
pub use config::show_config;
pub use mcp::{mcp_add, mcp_list, mcp_remove};
pub use web_mode::run_web;
