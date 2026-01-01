use serde::{Deserialize, Serialize};

use crate::JSONRPC_VERSION;

fn default_jsonrpc() -> String {
    JSONRPC_VERSION.to_owned()
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
    Error(JsonRpcError),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub id: RequestId,
    #[serde(rename = "jsonrpc", default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JsonRpcNotification {
    #[serde(rename = "jsonrpc", default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JsonRpcResponse {
    pub id: RequestId,
    #[serde(rename = "jsonrpc", default = "default_jsonrpc")]
    pub jsonrpc: String,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub error: JsonRpcErrorData,
    pub id: RequestId,
    #[serde(rename = "jsonrpc", default = "default_jsonrpc")]
    pub jsonrpc: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JsonRpcErrorData {
    pub code: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub message: String,
}

impl JsonRpcRequest {
    pub fn new(id: RequestId, method: impl Into<String>) -> Self {
        Self {
            id,
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: method.into(),
            params: None,
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: method.into(),
            params: None,
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }
}

impl JsonRpcResponse {
    pub fn new(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            id,
            jsonrpc: JSONRPC_VERSION.to_owned(),
            result,
        }
    }
}

impl JsonRpcError {
    pub fn new(id: RequestId, code: i64, message: impl Into<String>) -> Self {
        Self {
            id,
            jsonrpc: JSONRPC_VERSION.to_owned(),
            error: JsonRpcErrorData {
                code,
                message: message.into(),
                data: None,
            },
        }
    }
}
