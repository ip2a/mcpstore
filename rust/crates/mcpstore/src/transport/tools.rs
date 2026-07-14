use rmcp::model::CallToolRequestParams;

use crate::transport::client::McpConnection;
use crate::transport::content::content_item_from_rmcp;
use crate::transport::{DiscoveredTool, Result, ToolCallResult, TransportError};

impl McpConnection {
    pub async fn list_tools(&self) -> Result<Vec<DiscoveredTool>> {
        let client = self.get_client()?;
        let resp = client
            .list_tools(None)
            .await
            .map_err(|err| TransportError::Protocol(format!("list_tools failed: {err}")))?;

        let tools = resp
            .tools
            .into_iter()
            .map(|tool| DiscoveredTool {
                name: tool.name.to_string(),
                title: tool.title,
                description: tool.description.unwrap_or_default().to_string(),
                input_schema: serde_json::to_value(&tool.input_schema).unwrap_or_default(),
                output_schema: tool
                    .output_schema
                    .as_ref()
                    .and_then(|schema| serde_json::to_value(schema).ok()),
                annotations: tool
                    .annotations
                    .as_ref()
                    .and_then(|annotations| serde_json::to_value(annotations).ok()),
                execution: tool
                    .execution
                    .as_ref()
                    .and_then(|execution| serde_json::to_value(execution).ok()),
                icons: tool
                    .icons
                    .as_ref()
                    .and_then(|icons| serde_json::to_value(icons).ok()),
                meta: tool
                    .meta
                    .as_ref()
                    .and_then(|meta| serde_json::to_value(meta).ok()),
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
            .collect::<Result<Vec<_>>>()?;

        Ok(ToolCallResult {
            content,
            is_error: resp.is_error.unwrap_or(false),
        })
    }
}
