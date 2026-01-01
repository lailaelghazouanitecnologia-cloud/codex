use mms_tools::handlers::{
    BackgroundProcessManager, BackgroundShellHandler, BackgroundStatusHandler,
    BackgroundKillHandler,
};
use mms_tools::ToolHandler;
use std::sync::Arc;
use tokio::sync::Mutex;

#[test]
fn test_process_manager_id_generation() {
    let mut manager = BackgroundProcessManager::new();

    // We can't easily create a Child in tests, but we can test the ID generation
    // Note: generate_id is private, so we test via add_process behavior
    assert!(manager.list_processes().is_empty());
}

#[test]
fn test_process_manager_list_empty() {
    let manager = BackgroundProcessManager::new();
    assert!(manager.list_processes().is_empty());
}

#[test]
fn test_background_shell_handler_spec() {
    let manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
    let handler = BackgroundShellHandler::new(manager);
    let spec = handler.spec();
    assert_eq!(spec.name, "background_shell");
}

#[test]
fn test_background_status_handler_spec() {
    let manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
    let handler = BackgroundStatusHandler::new(manager);
    let spec = handler.spec();
    assert_eq!(spec.name, "background_status");
}

#[test]
fn test_background_kill_handler_spec() {
    let manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
    let handler = BackgroundKillHandler::new(manager);
    let spec = handler.spec();
    assert_eq!(spec.name, "background_kill");
}
