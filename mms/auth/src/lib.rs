//! Authentication management for MMS agent.
//!
//! This crate provides authentication infrastructure supporting:
//! - API key authentication for various providers
//! - OAuth-based authentication (ChatGPT login)
//! - Multiple storage backends (file, keyring, auto)
//! - Token refresh and management
//!
//! # Example
//!
//! ```ignore
//! use mms_auth::{AuthManager, AuthStorageMode};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() {
//!     let home = PathBuf::from("/home/user/.mms");
//!     let auth = AuthManager::new(home, AuthStorageMode::Auto);
//!
//!     // Check status
//!     let status = auth.status().await.unwrap();
//!     if status.authenticated {
//!         let token = auth.get_token().await.unwrap();
//!         // Use token for API requests
//!     }
//! }
//! ```

#![deny(clippy::print_stdout, clippy::print_stderr)]
#![forbid(unsafe_code)]

pub mod manager;
pub mod provider;
pub mod storage;
pub mod token;

// Re-exports
pub use manager::{
    AuthError, AuthManager, AuthMode, AuthResult, AuthStatus, SharedAuthManager,
    new_auth_manager, API_KEY_ENV_VAR, ANTHROPIC_API_KEY_ENV_VAR, OPENAI_API_KEY_ENV_VAR,
};
pub use provider::{
    AuthSource, ProviderAuthStatus, ProviderConfig, WireApi,
    check_all_providers_auth, check_provider_auth,
};
pub use storage::{
    AuthStorage, AuthStorageMode, FileAuthStorage, StorageError, StorageResult, StoredAuth,
    auth_file_path, create_storage,
};
pub use token::{
    IdTokenError, IdTokenInfo, KnownPlan, PlanType, TokenData, TokenErrorResponse,
    TokenRefreshResponse, parse_id_token,
};
