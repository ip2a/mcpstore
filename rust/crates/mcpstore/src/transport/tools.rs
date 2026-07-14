use rmcp::model::CallToolRequestParams;

use crate::transport::client::McpConnection;
use crate::transport::content::content_item_from_rmcp;
use crate::transport::{DiscoveredTool, Result, ToolCallResult, TransportError};

impl McpConnection {
    pub async fn list_tools(&self) -> Result<Vec<DiscoveredTool>> {
        self.require_tools()?;
        let client = self.get_client()?;
        let resp = match client.list_tools(None).await {
            Ok(response) => response,
            Err(error) => {
                return Err(self
                    .classify_client_failure(TransportError::Protocol(format!(
                        "list_tools failed: {error}"
                    )))
                    .await);
            }
        };

        let tools = resp.tools.into_iter().map(DiscoveredTool::from).collect();
        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult> {
        self.require_tools()?;
        let client = self.get_client()?;

        let args_map = match arguments {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let param = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args_map);

        let resp = match client.call_tool(param).await {
            Ok(response) => response,
            Err(error) => {
                return Err(self
                    .classify_client_failure(TransportError::ToolCallFailed(error.to_string()))
                    .await);
            }
        };

        let content = resp
            .content
            .into_iter()
            .map(content_item_from_rmcp)
            .collect::<Result<Vec<_>>>()?;

        Ok(ToolCallResult {
            content,
            is_error: resp.is_error.unwrap_or(false),
        })
    }
}
