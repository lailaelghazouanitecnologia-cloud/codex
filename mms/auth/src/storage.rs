//! Authentication credential storage backends.
//!
//! This module provides different storage backends for persisting
//! authentication credentials:
//! - File-based storage in the MMS home directory
//! - System keyring storage (when available)
//! - Auto-detection with fallback

use std::fmt::Debug;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "keyring")]
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::warn;

use crate::token::TokenData;

/// Storage mode for authentication credentials.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthStorageMode {
    /// Persist credentials in MMS_HOME/auth.json.
    #[default]
    File,

    /// Persist credentials in the system keyring.
    /// Fails if keyring is unavailable.
    Keyring,

    /// Use keyring when available, otherwise fall back to file.
    Auto,
}

/// Stored authentication data structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredAuth {
    /// API key for direct API authentication.
    #[serde(rename = "API_KEY", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// OpenAI-specific API key.
    #[serde(rename = "OPENAI_API_KEY", skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,

    /// Anthropic-specific API key.
    #[serde(rename = "ANTHROPIC_API_KEY", skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,

    /// OAuth tokens for ChatGPT login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenData>,

    /// Last token refresh timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<DateTime<Utc>>,

    /// Active provider ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_provider: Option<String>,
}

impl StoredAuth {
    /// Create empty stored auth.
    pub fn empty() -> Self {
        Self {
            api_key: None,
            openai_api_key: None,
            anthropic_api_key: None,
            tokens: None,
            last_refresh: None,
            active_provider: None,
        }
    }

    /// Create stored auth with an API key.
    pub fn with_api_key(key: String) -> Self {
        Self {
            api_key: Some(key),
            ..Self::empty()
        }
    }

    /// Create stored auth with OAuth tokens.
    pub fn with_tokens(tokens: TokenData) -> Self {
        Self {
            tokens: Some(tokens),
            last_refresh: Some(Utc::now()),
            ..Self::empty()
        }
    }

    /// Check if any authentication is present.
    pub fn has_auth(&self) -> bool {
        self.api_key.is_some()
            || self.openai_api_key.is_some()
            || self.anthropic_api_key.is_some()
            || self.tokens.is_some()
    }

    /// Get the API key for the specified provider.
    pub fn api_key_for_provider(&self, provider: &str) -> Option<&str> {
        match provider.to_lowercase().as_str() {
            "openai" | "chatgpt" => self.openai_api_key.as_deref(),
            "anthropic" | "claude" => self.anthropic_api_key.as_deref(),
            _ => self.api_key.as_deref(),
        }
    }
}

impl Default for StoredAuth {
    fn default() -> Self {
        Self::empty()
    }
}

/// Errors that can occur in storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// IO error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Keyring access error.
    #[error("Keyring error: {0}")]
    Keyring(String),

    /// Storage not available.
    #[error("Storage not available: {0}")]
    NotAvailable(String),
}

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Trait for authentication storage backends.
pub trait AuthStorage: Debug + Send + Sync {
    /// Load stored authentication data.
    fn load(&self) -> StorageResult<Option<StoredAuth>>;

    /// Save authentication data.
    fn save(&self, auth: &StoredAuth) -> StorageResult<()>;

    /// Delete stored authentication data.
    fn delete(&self) -> StorageResult<bool>;
}

/// Get the path to the auth file in the given home directory.
pub fn auth_file_path(home: &Path) -> PathBuf {
    home.join("auth.json")
}

/// File-based authentication storage.
#[derive(Debug, Clone)]
pub struct FileAuthStorage {
    home_dir: PathBuf,
}

impl FileAuthStorage {
    /// Create a new file-based storage with the given home directory.
    pub fn new(home_dir: PathBuf) -> Self {
        Self { home_dir }
    }

    /// Get the auth file path.
    pub fn auth_file(&self) -> PathBuf {
        auth_file_path(&self.home_dir)
    }
}

impl AuthStorage for FileAuthStorage {
    fn load(&self) -> StorageResult<Option<StoredAuth>> {
        let auth_file = self.auth_file();

        match File::open(&auth_file) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let auth: StoredAuth = serde_json::from_str(&contents)?;
                Ok(Some(auth))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    fn save(&self, auth: &StoredAuth) -> StorageResult<()> {
        let auth_file = self.auth_file();

        // Ensure parent directory exists
        if let Some(parent) = auth_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write with restrictive permissions on Unix
        let json_data = serde_json::to_string_pretty(auth)?;
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600); // Owner read/write only
        }

        let mut file = options.open(&auth_file)?;
        file.write_all(json_data.as_bytes())?;
        file.flush()?;

        Ok(())
    }

    fn delete(&self) -> StorageResult<bool> {
        let auth_file = self.auth_file();

        match fs::remove_file(&auth_file) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(StorageError::Io(e)),
        }
    }
}

/// Keyring service name for MMS credentials.
#[cfg(feature = "keyring")]
const KEYRING_SERVICE: &str = "MMS Auth";

/// Compute a stable key for keyring storage based on home directory.
#[cfg(feature = "keyring")]
fn compute_keyring_key(home: &Path) -> String {
    let canonical = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    let path_str = canonical.to_string_lossy();

    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let digest = hasher.finalize();

    let hex = format!("{:x}", digest);
    let truncated = hex.get(..16).unwrap_or(&hex);

    format!("mms|{}", truncated)
}

/// Keyring-based authentication storage.
#[derive(Debug)]
pub struct KeyringAuthStorage {
    #[cfg(feature = "keyring")]
    home_dir: PathBuf,
    #[cfg(feature = "keyring")]
    entry: keyring::Entry,
}

#[cfg(feature = "keyring")]
impl KeyringAuthStorage {
    /// Create a new keyring-based storage.
    pub fn new(home_dir: PathBuf) -> StorageResult<Self> {
        let key = compute_keyring_key(&home_dir);
        let entry = keyring::Entry::new(KEYRING_SERVICE, &key)
            .map_err(|e| StorageError::Keyring(e.to_string()))?;

        Ok(Self { home_dir, entry })
    }
}

#[cfg(feature = "keyring")]
impl AuthStorage for KeyringAuthStorage {
    fn load(&self) -> StorageResult<Option<StoredAuth>> {
        match self.entry.get_password() {
            Ok(serialized) => {
                let auth: StoredAuth = serde_json::from_str(&serialized)?;
                Ok(Some(auth))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(StorageError::Keyring(e.to_string())),
        }
    }

    fn save(&self, auth: &StoredAuth) -> StorageResult<()> {
        let serialized = serde_json::to_string(auth)?;
        self.entry
            .set_password(&serialized)
            .map_err(|e| StorageError::Keyring(e.to_string()))?;

        // Delete file-based auth if it exists (migrating to keyring)
        let file_storage = FileAuthStorage::new(self.home_dir.clone());
        if let Err(e) = file_storage.delete() {
            warn!("Failed to remove file-based auth after keyring save: {}", e);
        }

        Ok(())
    }

    fn delete(&self) -> StorageResult<bool> {
        match self.entry.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(StorageError::Keyring(e.to_string())),
        }
    }
}

#[cfg(not(feature = "keyring"))]
impl KeyringAuthStorage {
    /// Create a new keyring-based storage (stub when keyring is disabled).
    pub fn new(_home_dir: PathBuf) -> StorageResult<Self> {
        Err(StorageError::NotAvailable(
            "Keyring support not compiled in".to_string(),
        ))
    }
}

#[cfg(not(feature = "keyring"))]
impl AuthStorage for KeyringAuthStorage {
    fn load(&self) -> StorageResult<Option<StoredAuth>> {
        Err(StorageError::NotAvailable(
            "Keyring support not compiled in".to_string(),
        ))
    }

    fn save(&self, _auth: &StoredAuth) -> StorageResult<()> {
        Err(StorageError::NotAvailable(
            "Keyring support not compiled in".to_string(),
        ))
    }

    fn delete(&self) -> StorageResult<bool> {
        Err(StorageError::NotAvailable(
            "Keyring support not compiled in".to_string(),
        ))
    }
}

/// Auto-detecting storage that tries keyring first, then falls back to file.
#[derive(Debug)]
pub struct AutoAuthStorage {
    file_storage: FileAuthStorage,
    #[cfg(feature = "keyring")]
    keyring_storage: Option<KeyringAuthStorage>,
}

impl AutoAuthStorage {
    /// Create a new auto-detecting storage.
    pub fn new(home_dir: PathBuf) -> Self {
        let file_storage = FileAuthStorage::new(home_dir.clone());

        #[cfg(feature = "keyring")]
        let keyring_storage = KeyringAuthStorage::new(home_dir).ok();

        #[cfg(not(feature = "keyring"))]
        let _ = home_dir; // Suppress unused warning

        Self {
            file_storage,
            #[cfg(feature = "keyring")]
            keyring_storage,
        }
    }
}

impl AuthStorage for AutoAuthStorage {
    fn load(&self) -> StorageResult<Option<StoredAuth>> {
        #[cfg(feature = "keyring")]
        if let Some(ref keyring) = self.keyring_storage {
            match keyring.load() {
                Ok(Some(auth)) => return Ok(Some(auth)),
                Ok(None) => {}
                Err(e) => {
                    warn!("Failed to load from keyring, falling back to file: {}", e);
                }
            }
        }

        self.file_storage.load()
    }

    fn save(&self, auth: &StoredAuth) -> StorageResult<()> {
        #[cfg(feature = "keyring")]
        if let Some(ref keyring) = self.keyring_storage {
            match keyring.save(auth) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("Failed to save to keyring, falling back to file: {}", e);
                }
            }
        }

        self.file_storage.save(auth)
    }

    fn delete(&self) -> StorageResult<bool> {
        let mut deleted = false;

        #[cfg(feature = "keyring")]
        if let Some(ref keyring) = self.keyring_storage {
            match keyring.delete() {
                Ok(true) => deleted = true,
                Ok(false) => {}
                Err(e) => {
                    warn!("Failed to delete from keyring: {}", e);
                }
            }
        }

        match self.file_storage.delete() {
            Ok(true) => deleted = true,
            Ok(false) => {}
            Err(e) => {
                warn!("Failed to delete file storage: {}", e);
            }
        }

        Ok(deleted)
    }
}

/// Create an auth storage backend based on the specified mode.
pub fn create_storage(mode: AuthStorageMode, home_dir: PathBuf) -> Arc<dyn AuthStorage> {
    match mode {
        AuthStorageMode::File => Arc::new(FileAuthStorage::new(home_dir)),
        AuthStorageMode::Keyring => {
            match KeyringAuthStorage::new(home_dir.clone()) {
                Ok(storage) => Arc::new(storage),
                Err(e) => {
                    warn!("Keyring not available, falling back to file: {}", e);
                    Arc::new(FileAuthStorage::new(home_dir))
                }
            }
        }
        AuthStorageMode::Auto => Arc::new(AutoAuthStorage::new(home_dir)),
    }
}
