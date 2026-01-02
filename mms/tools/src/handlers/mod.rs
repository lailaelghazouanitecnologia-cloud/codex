mod background;
mod edit_file;
mod glob;
mod grep;
mod list_dir;
mod read_file;
mod shell;
mod write_file;

pub use background::{
    BackgroundKillHandler, BackgroundProcessManager, BackgroundShellHandler,
    BackgroundStatusHandler, ProcessId, ProcessStatus,
};
pub use edit_file::EditFileHandler;
pub use glob::GlobHandler;
pub use grep::GrepHandler;
pub use list_dir::ListDirectoryHandler;
pub use read_file::ReadFileHandler;
pub use shell::ShellHandler;
pub use write_file::WriteFileHandler;

use crate::registry::ToolRegistry;
use std::sync::Arc;
use tokio::sync::Mutex;

pub fn register_default_handlers(registry: &mut ToolRegistry) {
    registry.register(ReadFileHandler);
    registry.register(WriteFileHandler);
    registry.register(EditFileHandler);
    registry.register(ListDirectoryHandler);
    registry.register(GlobHandler);
    registry.register(GrepHandler);
    registry.register(ShellHandler);
}

pub fn register_all_handlers(
    registry: &mut ToolRegistry,
    bg_manager: Arc<Mutex<BackgroundProcessManager>>,
) {
    registry.register(ReadFileHandler);
    registry.register(WriteFileHandler);
    registry.register(EditFileHandler);
    registry.register(ListDirectoryHandler);
    registry.register(GlobHandler);
    registry.register(GrepHandler);
    registry.register(ShellHandler);
    registry.register(BackgroundShellHandler::new(bg_manager.clone()));
    registry.register(BackgroundStatusHandler::new(bg_manager.clone()));
    registry.register(BackgroundKillHandler::new(bg_manager));
}
