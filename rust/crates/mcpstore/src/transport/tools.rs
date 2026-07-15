use crate::transport::client::McpConnection;
use crate::transport::{
    DiscoveredTool, McpExecutionOptions, McpToolExecution, Result, ToolCallResult, TransportError,
};

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
        match self
            .start_tool_call(tool_name, arguments, McpExecutionOptions::default())
            .await?
            .wait()
            .await?
        {
            McpToolExecution::Immediate { result } => Ok(result),
            McpToolExecution::Task { .. } => Err(TransportError::Protocol(
                "tool call unexpectedly returned a task".to_string(),
            )),
        }
    }
}
