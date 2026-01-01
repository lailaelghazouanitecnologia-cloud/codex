use serde::{Deserialize, Serialize};

use crate::content::ContentBlock;
use crate::request::Implementation;
use crate::resource::{Resource, ResourceTemplate};
use crate::tool::Tool;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "serverInfo")]
    pub server_info: Implementation,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<ServerCapabilitiesPrompts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ServerCapabilitiesResources>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ServerCapabilitiesTools>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ServerCapabilitiesTools {
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ServerCapabilitiesResources {
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ServerCapabilitiesPrompts {
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ListToolsResult {
    #[serde(rename = "nextCursor", default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    #[serde(rename = "isError", default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(rename = "structuredContent", default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ListResourcesResult {
    #[serde(rename = "nextCursor", default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ListResourceTemplatesResult {
    #[serde(rename = "nextCursor", default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(rename = "resourceTemplates")]
    pub resource_templates: Vec<ResourceTemplate>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ReadResourceResult {
    pub contents: Vec<ResourceContents>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ResourceContents {
    Text(TextResourceContents),
    Blob(BlobResourceContents),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextResourceContents {
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BlobResourceContents {
    pub blob: String,
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub uri: String,
}

impl CallToolResult {
    pub fn success(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            is_error: Some(false),
            structured_content: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::text(message)],
            is_error: Some(true),
            structured_content: None,
        }
    }
}
