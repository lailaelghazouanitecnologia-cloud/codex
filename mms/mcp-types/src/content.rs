use serde::{Deserialize, Serialize};

use crate::resource::Annotations;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Text(TextContent),
    Image(ImageContent),
    Audio(AudioContent),
    Resource(ResourceLink),
    Embedded(EmbeddedResource),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    pub text: String,
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ImageContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    pub data: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AudioContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    pub data: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ResourceLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub r#type: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EmbeddedResource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
    pub resource: EmbeddedResourceContent,
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EmbeddedResourceContent {
    Text(crate::response::TextResourceContents),
    Blob(crate::response::BlobResourceContents),
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(TextContent {
            annotations: None,
            text: text.into(),
            r#type: "text".to_string(),
        })
    }

    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image(ImageContent {
            annotations: None,
            data: data.into(),
            mime_type: mime_type.into(),
            r#type: "image".to_string(),
        })
    }
}

impl TextContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            annotations: None,
            text: text.into(),
            r#type: "text".to_string(),
        }
    }
}
