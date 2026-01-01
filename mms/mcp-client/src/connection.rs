use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::{anyhow, Result};
use mms_mcp_types::{
    CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult,
    InitializeRequestParams, InitializeResult, JsonRpcMessage, JsonRpcNotification,
    JsonRpcRequest, ListPromptsResult, ListResourcesResult, ListToolsResult,
    ReadResourceRequestParams, ReadResourceResult, RequestId,
};
use tokio::sync::Mutex;

use crate::transport::Transport;

pub struct McpConnection<T: Transport> {
    transport: Mutex<T>,
    request_id: AtomicI64,
    server_info: Mutex<Option<InitializeResult>>,
}

impl<T: Transport> McpConnection<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport: Mutex::new(transport),
            request_id: AtomicI64::new(1),
            server_info: Mutex::new(None),
        }
    }

    fn next_request_id(&self) -> RequestId {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        RequestId::Integer(id)
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = self.next_request_id();
        let request = JsonRpcRequest::new(id.clone(), method).with_params(params);

        let transport = self.transport.lock().await;
        transport.send(JsonRpcMessage::Request(request)).await?;

        loop {
            let response = transport.receive().await?;

            match response {
                Some(JsonRpcMessage::Response(resp)) => {
                    if resp.id == id {
                        return Ok(resp.result);
                    }
                }
                Some(JsonRpcMessage::Error(err)) => {
                    if err.id == id {
                        return Err(anyhow!(
                            "RPC error {}: {}",
                            err.error.code,
                            err.error.message
                        ));
                    }
                }
                Some(JsonRpcMessage::Notification(_)) => {
                    continue;
                }
                Some(JsonRpcMessage::Request(_)) => {
                    continue;
                }
                None => {
                    return Err(anyhow!("Connection closed"));
                }
            }
        }
    }

    pub async fn initialize(&self, params: InitializeRequestParams) -> Result<InitializeResult> {
        let params_json = serde_json::to_value(&params)?;
        let result = self.send_request("initialize", params_json).await?;
        let init_result: InitializeResult = serde_json::from_value(result)?;

        let mut server_info = self.server_info.lock().await;
        *server_info = Some(init_result.clone());

        Ok(init_result)
    }

    pub async fn list_tools(&self) -> Result<ListToolsResult> {
        let result = self.send_request("tools/list", serde_json::json!({})).await?;
        let tools: ListToolsResult = serde_json::from_value(result)?;
        Ok(tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: Option<serde_json::Value>) -> Result<CallToolResult> {
        let params = CallToolRequestParams {
            name: name.to_string(),
            arguments,
        };
        let params_json = serde_json::to_value(&params)?;
        let result = self.send_request("tools/call", params_json).await?;
        let call_result: CallToolResult = serde_json::from_value(result)?;
        Ok(call_result)
    }

    pub async fn list_resources(&self) -> Result<ListResourcesResult> {
        let result = self.send_request("resources/list", serde_json::json!({})).await?;
        let resources: ListResourcesResult = serde_json::from_value(result)?;
        Ok(resources)
    }

    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult> {
        let params = ReadResourceRequestParams {
            uri: uri.to_string(),
        };
        let params_json = serde_json::to_value(&params)?;
        let result = self.send_request("resources/read", params_json).await?;
        let read_result: ReadResourceResult = serde_json::from_value(result)?;
        Ok(read_result)
    }

    pub async fn list_prompts(&self) -> Result<ListPromptsResult> {
        let result = self.send_request("prompts/list", serde_json::json!({})).await?;
        let prompts: ListPromptsResult = serde_json::from_value(result)?;
        Ok(prompts)
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<GetPromptResult> {
        let params = GetPromptRequestParams {
            name: name.to_string(),
            arguments,
        };
        let params_json = serde_json::to_value(&params)?;
        let result = self.send_request("prompts/get", params_json).await?;
        let prompt_result: GetPromptResult = serde_json::from_value(result)?;
        Ok(prompt_result)
    }

    pub async fn ping(&self) -> Result<()> {
        self.send_request("ping", serde_json::json!({})).await?;
        Ok(())
    }

    pub async fn subscribe_resource(&self, uri: &str) -> Result<()> {
        let params = serde_json::json!({ "uri": uri });
        self.send_request("resources/subscribe", params).await?;
        Ok(())
    }

    pub async fn unsubscribe_resource(&self, uri: &str) -> Result<()> {
        let params = serde_json::json!({ "uri": uri });
        self.send_request("resources/unsubscribe", params).await?;
        Ok(())
    }

    pub async fn send_notification(&self, method: &str, params: serde_json::Value) -> Result<()> {
        let notification = JsonRpcNotification::new(method).with_params(params);
        let transport = self.transport.lock().await;
        transport.send(JsonRpcMessage::Notification(notification)).await?;
        Ok(())
    }

    pub async fn notify_cancelled(&self, request_id: RequestId, reason: Option<String>) -> Result<()> {
        let params = serde_json::json!({
            "requestId": request_id,
            "reason": reason
        });
        self.send_notification("notifications/cancelled", params).await
    }

    pub async fn notify_progress(
        &self,
        progress_token: serde_json::Value,
        progress: f64,
        total: Option<f64>,
    ) -> Result<()> {
        let params = serde_json::json!({
            "progressToken": progress_token,
            "progress": progress,
            "total": total
        });
        self.send_notification("notifications/progress", params).await
    }

    pub async fn server_info(&self) -> Option<InitializeResult> {
        let guard = self.server_info.lock().await;
        guard.clone()
    }

    pub async fn is_initialized(&self) -> bool {
        let guard = self.server_info.lock().await;
        guard.is_some()
    }

    pub async fn has_capability(&self, capability: &str) -> bool {
        let guard = self.server_info.lock().await;
        if let Some(info) = guard.as_ref() {
            let caps = &info.capabilities;
            match capability {
                "tools" => caps.tools.is_some(),
                "resources" => caps.resources.is_some(),
                "prompts" => caps.prompts.is_some(),
                _ => false,
            }
        } else {
            false
        }
    }
}
