use rmcp::model::CallToolRequestParams;

use crate::transport::client::McpConnection;
use crate::transport::content::content_item_from_rmcp;
use crate::transport::{Result, ToolCallResult, ToolDescription, TransportError};

impl McpConnection {
    pub async fn list_tools(&self) -> Result<Vec<ToolDescription>> {
        let client = self.get_client()?;
        let resp = client
            .list_tools(None)
            .await
            .map_err(|err| TransportError::Protocol(format!("list_tools failed: {err}")))?;

        let tools = resp
            .tools
            .into_iter()
            .map(|tool| ToolDescription {
                name: tool.name.to_string(),
                description: tool.description.unwrap_or_default().to_string(),
                input_schema: serde_json::to_value(&tool.input_schema).unwrap_or_default(),
            })
            .collect();
        Ok(tools)
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult> {
        let client = self.get_client()?;

        let args_map = match arguments {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let param = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args_map);

        let resp = client
            .call_tool(param)
            .await
            .map_err(|err| TransportError::ToolCallFailed(format!("{err}")))?;

        let content = resp
            .content
            .into_iter()
            .map(content_item_from_rmcp)
            .collect();

        Ok(ToolCallResult {
            content,
            is_error: resp.is_error.unwrap_or(false),
        })
    }
}
