#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

mod error;
mod result;
mod types;

pub use error::AgentError;
pub use result::AgentResult;
pub use types::*;
