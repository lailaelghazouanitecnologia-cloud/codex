//! Authentication manager for handling auth state and token refresh.
//!
//! The AuthManager provides a high-level interface for authentication,
//! supporting both API key and OAuth-based authentication flows.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::storage::{AuthStorage, AuthStorageMode, StoredAuth, create_storage};
use crate::token::{TokenData, TokenRefreshResponse, parse_id_token};

/// Token refresh interval in days.
const TOKEN_REFRESH_INTERVAL_DAYS: i64 = 8;

/// OAuth client ID for OpenAI.
const OPENAI_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Token refresh URL for OpenAI.
const OPENAI_REFRESH_URL: &str = "https://auth.openai.com/oauth/token";

/// Environment variable for API key.
pub const API_KEY_ENV_VAR: &str = "MMS_API_KEY";

/// Environment variable for OpenAI API key.
pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";

/// Environment variable for Anthropic API key.
pub const ANTHROPIC_API_KEY_ENV_VAR: &str = "ANTHROPIC_API_KEY";

/// Authentication mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    /// No authentication configured.
    #[default]
    None,

    /// API key authentication.
    ApiKey,

    /// OAuth-based authentication (ChatGPT login).
    OAuth,
}

/// Result of an authentication check.
#[derive(Debug, Clone)]
pub struct AuthStatus {
    /// Current authentication mode.
    pub mode: AuthMode,

    /// Whether the user is authenticated.
    pub authenticated: bool,

    /// User's email (if available from OAuth).
    pub email: Option<String>,

    /// Subscription plan type (if available from OAuth).
    pub plan: Option<String>,

    /// Account/workspace ID (if available).
    pub account_id: Option<String>,

    /// Active provider ID.
    pub provider: Option<String>,
}

impl AuthStatus {
    /// Create an unauthenticated status.
    pub fn unauthenticated() -> Self {
        Self {
            mode: AuthMode::None,
            authenticated: false,
            email: None,
            plan: None,
            account_id: None,
            provider: None,
        }
    }
}

/// Errors that can occur during authentication operations.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Storage operation failed.
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::StorageError),

    /// Token refresh failed permanently.
    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),

    /// Token refresh failed transiently (network issue).
    #[error("Token refresh failed (transient): {0}")]
    RefreshTransient(String),

    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Token parsing failed.
    #[error("Token parsing error: {0}")]
    TokenParse(#[from] crate::token::IdTokenError),

    /// No authentication configured.
    #[error("Not authenticated")]
    NotAuthenticated,

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for auth operations.
pub type AuthResult<T> = Result<T, AuthError>;

/// Cached authentication state.
#[derive(Debug, Default)]
struct CachedAuth {
    stored: Option<StoredAuth>,
    mode: AuthMode,
    last_loaded: Option<chrono::DateTime<Utc>>,
}

/// Authentication manager for handling auth state and token operations.
#[derive(Debug)]
pub struct AuthManager {
    /// Home directory for auth storage.
    home_dir: PathBuf,

    /// Storage backend.
    storage: Arc<dyn AuthStorage>,

    /// HTTP client for token refresh.
    http_client: reqwest::Client,

    /// Cached authentication state.
    cache: RwLock<CachedAuth>,

    /// Whether to check environment variables for API keys.
    check_env: bool,
}

impl AuthManager {
    /// Create a new auth manager.
    pub fn new(home_dir: PathBuf, storage_mode: AuthStorageMode) -> Self {
        let storage = create_storage(storage_mode, home_dir.clone());

        Self {
            home_dir,
            storage,
            http_client: reqwest::Client::new(),
            cache: RwLock::new(CachedAuth::default()),
            check_env: true,
        }
    }

    /// Create an auth manager with file-based storage (default).
    pub fn with_file_storage(home_dir: PathBuf) -> Self {
        Self::new(home_dir, AuthStorageMode::File)
    }

    /// Create an auth manager with auto-detection storage.
    pub fn with_auto_storage(home_dir: PathBuf) -> Self {
        Self::new(home_dir, AuthStorageMode::Auto)
    }

    /// Set whether to check environment variables for API keys.
    pub fn set_check_env(&mut self, check: bool) {
        self.check_env = check;
    }

    /// Load authentication from storage.
    pub async fn load(&self) -> AuthResult<Option<StoredAuth>> {
        let stored = self.storage.load()?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.stored = stored.clone();
        cache.mode = self.determine_mode(&stored);
        cache.last_loaded = Some(Utc::now());

        Ok(stored)
    }

    /// Get the current authentication status.
    pub async fn status(&self) -> AuthResult<AuthStatus> {
        // First try to get cached auth
        let cache = self.cache.read().await;
        if let Some(ref stored) = cache.stored {
            return Ok(self.status_from_stored(stored));
        }
        drop(cache);

        // Load from storage
        let stored = self.load().await?;

        if let Some(stored) = stored {
            return Ok(self.status_from_stored(&stored));
        }

        // Check environment variables
        if self.check_env {
            if let Some(status) = self.status_from_env() {
                return Ok(status);
            }
        }

        Ok(AuthStatus::unauthenticated())
    }

    /// Get the bearer token for API requests.
    ///
    /// This will automatically refresh the token if needed.
    pub async fn get_token(&self) -> AuthResult<String> {
        // Check environment first
        if self.check_env {
            if let Ok(key) = std::env::var(OPENAI_API_KEY_ENV_VAR) {
                return Ok(key);
            }
            if let Ok(key) = std::env::var(ANTHROPIC_API_KEY_ENV_VAR) {
                return Ok(key);
            }
            if let Ok(key) = std::env::var(API_KEY_ENV_VAR) {
                return Ok(key);
            }
        }

        // Get cached auth
        let cache = self.cache.read().await;
        let stored = cache.stored.clone();
        drop(cache);

        let stored = match stored {
            Some(s) => s,
            None => self.load().await?.ok_or(AuthError::NotAuthenticated)?,
        };

        // If we have an API key, use it
        if let Some(key) = &stored.api_key {
            return Ok(key.clone());
        }

        if let Some(key) = &stored.openai_api_key {
            return Ok(key.clone());
        }

        // If we have OAuth tokens, check if refresh is needed
        if let Some(tokens) = &stored.tokens {
            let needs_refresh = stored.last_refresh.map_or(true, |last| {
                last < Utc::now() - chrono::Duration::days(TOKEN_REFRESH_INTERVAL_DAYS)
            });

            if needs_refresh {
                debug!("Token needs refresh, refreshing...");
                let new_token = self.refresh_token(&tokens.refresh_token).await?;
                return Ok(new_token);
            }

            return Ok(tokens.access_token.clone());
        }

        Err(AuthError::NotAuthenticated)
    }

    /// Get the API key for a specific provider.
    pub async fn get_provider_key(&self, provider: &str) -> AuthResult<Option<String>> {
        // Check environment first
        if self.check_env {
            let env_var = match provider.to_lowercase().as_str() {
                "openai" | "chatgpt" => OPENAI_API_KEY_ENV_VAR,
                "anthropic" | "claude" => ANTHROPIC_API_KEY_ENV_VAR,
                _ => API_KEY_ENV_VAR,
            };

            if let Ok(key) = std::env::var(env_var) {
                return Ok(Some(key));
            }
        }

        // Get from storage
        let cache = self.cache.read().await;
        if let Some(ref stored) = cache.stored {
            return Ok(stored.api_key_for_provider(provider).map(String::from));
        }
        drop(cache);

        let stored = self.load().await?;
        Ok(stored.and_then(|s| s.api_key_for_provider(provider).map(String::from)))
    }

    /// Login with an API key.
    pub async fn login_api_key(&self, provider: &str, key: String) -> AuthResult<()> {
        let mut stored = self.load().await?.unwrap_or_default();

        match provider.to_lowercase().as_str() {
            "openai" | "chatgpt" => {
                stored.openai_api_key = Some(key);
            }
            "anthropic" | "claude" => {
                stored.anthropic_api_key = Some(key);
            }
            _ => {
                stored.api_key = Some(key);
            }
        }

        stored.active_provider = Some(provider.to_string());

        self.storage.save(&stored)?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.stored = Some(stored);
        cache.mode = AuthMode::ApiKey;
        cache.last_loaded = Some(Utc::now());

        info!("Logged in with API key for provider: {}", provider);
        Ok(())
    }

    /// Login with OAuth tokens (from ChatGPT login flow).
    pub async fn login_oauth(&self, tokens: TokenData) -> AuthResult<()> {
        let stored = StoredAuth::with_tokens(tokens);

        self.storage.save(&stored)?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.stored = Some(stored);
        cache.mode = AuthMode::OAuth;
        cache.last_loaded = Some(Utc::now());

        info!("Logged in with OAuth");
        Ok(())
    }

    /// Logout and clear stored credentials.
    pub async fn logout(&self) -> AuthResult<bool> {
        let deleted = self.storage.delete()?;

        // Clear cache
        let mut cache = self.cache.write().await;
        cache.stored = None;
        cache.mode = AuthMode::None;
        cache.last_loaded = None;

        if deleted {
            info!("Logged out successfully");
        }

        Ok(deleted)
    }

    /// Refresh the OAuth token.
    pub async fn refresh_token(&self, refresh_token: &str) -> AuthResult<String> {
        info!("Refreshing OAuth token");

        let response = self
            .http_client
            .post(OPENAI_REFRESH_URL)
            .timeout(Duration::from_secs(60))
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", OPENAI_CLIENT_ID),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // Check for permanent errors
            if status.as_u16() == 400 || status.as_u16() == 401 {
                return Err(AuthError::RefreshFailed(format!(
                    "Token refresh failed permanently ({}): {}",
                    status, body
                )));
            }

            return Err(AuthError::RefreshTransient(format!(
                "Token refresh failed ({}): {}",
                status, body
            )));
        }

        let refresh_response: TokenRefreshResponse = response.json().await?;

        // Parse the new ID token
        let id_token_info = parse_id_token(&refresh_response.id_token)?;

        // Create new token data
        let new_tokens = TokenData {
            id_token: id_token_info,
            access_token: refresh_response.access_token.clone(),
            refresh_token: refresh_response.refresh_token.unwrap_or_else(|| refresh_token.to_string()),
            account_id: None,
        };

        // Save updated tokens
        let mut stored = self.load().await?.unwrap_or_default();
        stored.tokens = Some(new_tokens.clone());
        stored.last_refresh = Some(Utc::now());

        self.storage.save(&stored)?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.stored = Some(stored);
        cache.last_loaded = Some(Utc::now());

        Ok(refresh_response.access_token)
    }

    /// Get the home directory.
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    /// Determine the auth mode from stored auth.
    fn determine_mode(&self, stored: &Option<StoredAuth>) -> AuthMode {
        match stored {
            Some(s) if s.tokens.is_some() => AuthMode::OAuth,
            Some(s) if s.has_auth() => AuthMode::ApiKey,
            _ => AuthMode::None,
        }
    }

    /// Create status from stored auth.
    fn status_from_stored(&self, stored: &StoredAuth) -> AuthStatus {
        let mode = if stored.tokens.is_some() {
            AuthMode::OAuth
        } else if stored.has_auth() {
            AuthMode::ApiKey
        } else {
            AuthMode::None
        };

        let (email, plan, account_id) = if let Some(tokens) = &stored.tokens {
            (
                tokens.id_token.email.clone(),
                tokens.id_token.plan_type_string(),
                tokens.id_token.account_id.clone(),
            )
        } else {
            (None, None, None)
        };

        AuthStatus {
            mode,
            authenticated: stored.has_auth(),
            email,
            plan,
            account_id,
            provider: stored.active_provider.clone(),
        }
    }

    /// Create status from environment variables.
    fn status_from_env(&self) -> Option<AuthStatus> {
        let (provider, found) = if std::env::var(OPENAI_API_KEY_ENV_VAR).is_ok() {
            (Some("openai".to_string()), true)
        } else if std::env::var(ANTHROPIC_API_KEY_ENV_VAR).is_ok() {
            (Some("anthropic".to_string()), true)
        } else if std::env::var(API_KEY_ENV_VAR).is_ok() {
            (None, true)
        } else {
            (None, false)
        };

        if found {
            Some(AuthStatus {
                mode: AuthMode::ApiKey,
                authenticated: true,
                email: None,
                plan: None,
                account_id: None,
                provider,
            })
        } else {
            None
        }
    }
}

/// Create a shared auth manager.
pub type SharedAuthManager = Arc<AuthManager>;

/// Create a new shared auth manager.
pub fn new_auth_manager(home_dir: PathBuf) -> SharedAuthManager {
    Arc::new(AuthManager::with_auto_storage(home_dir))
}
