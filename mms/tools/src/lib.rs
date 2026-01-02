#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod context;
mod handler;
pub mod mcp;
mod registry;
mod router;
mod spec;

pub mod handlers;

pub use context::*;
pub use handler::*;
pub use mcp::{McpManager, McpServerEntry, McpServersConfig};
pub use registry::*;
pub use router::*;
pub use spec::*;
