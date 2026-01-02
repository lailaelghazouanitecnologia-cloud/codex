//! Token data types and JWT parsing.
//!
//! This module handles OAuth token data including JWT parsing
//! for extracting user information and subscription details.

use base64::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Token data containing OAuth tokens and parsed JWT claims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenData {
    /// Parsed information from the ID token JWT.
    #[serde(
        deserialize_with = "deserialize_id_token",
        serialize_with = "serialize_id_token"
    )]
    pub id_token: IdTokenInfo,

    /// OAuth access token (JWT format).
    pub access_token: String,

    /// OAuth refresh token for obtaining new access tokens.
    pub refresh_token: String,

    /// Optional account identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

impl Default for TokenData {
    fn default() -> Self {
        Self {
            id_token: IdTokenInfo::default(),
            access_token: String::new(),
            refresh_token: String::new(),
            account_id: None,
        }
    }
}

/// Parsed claims from the ID token JWT.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IdTokenInfo {
    /// User's email address, if present.
    pub email: Option<String>,

    /// Subscription plan type (e.g., free, plus, pro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<PlanType>,

    /// Organization/workspace identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,

    /// The raw JWT string for serialization.
    #[serde(skip)]
    pub raw_jwt: String,
}

impl IdTokenInfo {
    /// Get the plan type as a display string.
    pub fn plan_type_string(&self) -> Option<String> {
        self.plan_type.as_ref().map(|p| p.to_string())
    }
}

/// Subscription plan type from token claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlanType {
    /// Known plan type.
    Known(KnownPlan),
    /// Unknown plan type (forward compatibility).
    Unknown(String),
}

impl std::fmt::Display for PlanType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanType::Known(plan) => write!(f, "{:?}", plan),
            PlanType::Unknown(s) => write!(f, "{}", s),
        }
    }
}

/// Known subscription plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownPlan {
    Free,
    Plus,
    Pro,
    Team,
    Business,
    Enterprise,
    Edu,
}

/// Errors that can occur when parsing ID tokens.
#[derive(Debug, Error)]
pub enum IdTokenError {
    /// The JWT format is invalid.
    #[error("invalid ID token format: expected header.payload.signature")]
    InvalidFormat,

    /// Base64 decoding failed.
    #[error("failed to decode base64: {0}")]
    Base64(#[from] base64::DecodeError),

    /// JSON parsing failed.
    #[error("failed to parse JWT claims: {0}")]
    Json(#[from] serde_json::Error),
}

/// JWT claims structure for parsing.
#[derive(Deserialize)]
struct JwtClaims {
    #[serde(default)]
    email: Option<String>,

    /// OpenAI-specific auth claims.
    #[serde(rename = "https://api.openai.com/auth", default)]
    openai_auth: Option<OpenAiAuthClaims>,

    /// Anthropic-specific claims.
    #[serde(rename = "https://api.anthropic.com/auth", default)]
    anthropic_auth: Option<AnthropicAuthClaims>,
}

/// OpenAI-specific JWT claims.
#[derive(Deserialize)]
struct OpenAiAuthClaims {
    #[serde(default)]
    chatgpt_plan_type: Option<PlanType>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

/// Anthropic-specific JWT claims (placeholder for future).
#[derive(Deserialize)]
struct AnthropicAuthClaims {
    #[serde(default)]
    plan_type: Option<PlanType>,
    #[serde(default)]
    account_id: Option<String>,
}

/// Parse an ID token JWT to extract claims.
///
/// This function decodes the JWT payload (without signature verification)
/// to extract user information like email and subscription plan.
pub fn parse_id_token(id_token: &str) -> Result<IdTokenInfo, IdTokenError> {
    // JWT format: header.payload.signature
    let parts: Vec<&str> = id_token.split('.').collect();

    if parts.len() != 3 {
        return Err(IdTokenError::InvalidFormat);
    }

    let payload_b64 = parts[1];
    if payload_b64.is_empty() {
        return Err(IdTokenError::InvalidFormat);
    }

    // Decode the payload (URL-safe base64 without padding)
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64)?;
    let claims: JwtClaims = serde_json::from_slice(&payload_bytes)?;

    // Extract plan type and account ID from provider-specific claims
    let (plan_type, account_id) = if let Some(openai) = claims.openai_auth {
        (openai.chatgpt_plan_type, openai.chatgpt_account_id)
    } else if let Some(anthropic) = claims.anthropic_auth {
        (anthropic.plan_type, anthropic.account_id)
    } else {
        (None, None)
    };

    Ok(IdTokenInfo {
        email: claims.email,
        plan_type,
        account_id,
        raw_jwt: id_token.to_string(),
    })
}

/// Custom deserializer for ID token that parses JWT.
fn deserialize_id_token<'de, D>(deserializer: D) -> Result<IdTokenInfo, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    parse_id_token(&s).map_err(serde::de::Error::custom)
}

/// Custom serializer for ID token that outputs raw JWT.
fn serialize_id_token<S>(id_token: &IdTokenInfo, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&id_token.raw_jwt)
}

/// Response from OAuth token refresh endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenRefreshResponse {
    /// New ID token.
    pub id_token: String,

    /// New access token.
    pub access_token: String,

    /// New refresh token (if rotated).
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// Token type (usually "Bearer").
    #[serde(default)]
    pub token_type: Option<String>,

    /// Expiration time in seconds.
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// Error response from OAuth token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenErrorResponse {
    /// Error code.
    pub error: String,

    /// Human-readable error description.
    #[serde(default)]
    pub error_description: Option<String>,
}
