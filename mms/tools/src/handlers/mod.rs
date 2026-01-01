mod background;
mod read_file;
mod shell;
mod write_file;

pub use background::{
    BackgroundProcessManager, BackgroundShellHandler, BackgroundStatusHandler,
    BackgroundKillHandler, ProcessId, ProcessStatus,
};
pub use read_file::ReadFileHandler;
pub use shell::ShellHandler;
pub use write_file::WriteFileHandler;

use crate::registry::ToolRegistry;
use std::sync::Arc;
use tokio::sync::Mutex;

pub fn register_default_handlers(registry: &mut ToolRegistry) {
    registry.register(ReadFileHandler);
    registry.register(WriteFileHandler);
    registry.register(ShellHandler);
}

pub fn register_all_handlers(
    registry: &mut ToolRegistry,
    bg_manager: Arc<Mutex<BackgroundProcessManager>>,
) {
    registry.register(ReadFileHandler);
    registry.register(WriteFileHandler);
    registry.register(ShellHandler);
    registry.register(BackgroundShellHandler::new(bg_manager.clone()));
    registry.register(BackgroundStatusHandler::new(bg_manager.clone()));
    registry.register(BackgroundKillHandler::new(bg_manager));
}
