use rmcp::model::{ClientRequest, ListToolsRequest, PaginatedRequestParams, ServerResult};

use crate::transport::client::McpConnection;
use crate::transport::protocol::send_protocol_request;
use crate::transport::{
    DiscoveredTool, McpExecutionOptions, McpToolExecution, Result, ToolCallResult, TransportError,
};

impl McpConnection {
    pub async fn list_tools(&self) -> Result<Vec<DiscoveredTool>> {
        self.require_tools()?;
        let mut tools = Vec::new();
        let mut cursor = None;
        loop {
            let request =
                ListToolsRequest::with_param(PaginatedRequestParams::default().with_cursor(cursor));
            let result = send_protocol_request(
                self.get_client()?,
                self.instance_id(),
                ClientRequest::ListToolsRequest(request),
                "list tools",
            )
            .await;
            let page = match result {
                Ok(ServerResult::ListToolsResult(page)) => page,
                Ok(_) => {
                    return Err(TransportError::Protocol(
                        "list tools returned an unexpected response".to_string(),
                    ))
                }
                Err(error) => return Err(self.classify_client_failure(error).await),
            };
            tools.extend(page.tools);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        Ok(tools.into_iter().map(DiscoveredTool::from).collect())
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
