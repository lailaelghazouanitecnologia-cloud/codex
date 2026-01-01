use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Tool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: ToolInputSchema,
    pub name: String,
    #[serde(rename = "outputSchema", default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<ToolOutputSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ToolInputSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(default = "default_object_type")]
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ToolOutputSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(default = "default_object_type")]
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ToolAnnotations {
    #[serde(rename = "destructiveHint", default, skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    #[serde(rename = "idempotentHint", default, skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    #[serde(rename = "openWorldHint", default, skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
    #[serde(rename = "readOnlyHint", default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

fn default_object_type() -> String {
    "object".to_string()
}

impl Tool {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            annotations: None,
            description: None,
            input_schema: ToolInputSchema::default(),
            name: name.into(),
            output_schema: None,
            title: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_input_schema(mut self, schema: ToolInputSchema) -> Self {
        self.input_schema = schema;
        self
    }
}

impl Default for ToolInputSchema {
    fn default() -> Self {
        Self {
            properties: None,
            required: None,
            r#type: default_object_type(),
        }
    }
}

impl ToolInputSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_properties(mut self, properties: serde_json::Value) -> Self {
        self.properties = Some(properties);
        self
    }

    pub fn with_required(mut self, required: Vec<String>) -> Self {
        self.required = Some(required);
        self
    }
}
