use rmcp::model::{
    ClientRequest, GetPromptRequest, GetPromptRequestParams, ListPromptsRequest,
    PaginatedRequestParams, ServerResult,
};

use crate::transport::client::McpConnection;
use crate::transport::protocol::send_protocol_request;
use crate::transport::{DiscoveredPrompt, Result, TransportError};

impl McpConnection {
    pub async fn list_prompts(&self) -> Result<Vec<DiscoveredPrompt>> {
        self.require_prompts()?;
        let mut prompts = Vec::new();
        let mut cursor = None;
        loop {
            let request = ListPromptsRequest::with_param(
                PaginatedRequestParams::default().with_cursor(cursor),
            );
            let result = send_protocol_request(
                self.get_client()?,
                self.instance_id(),
                ClientRequest::ListPromptsRequest(request),
                "list prompts",
            )
            .await;
            let page = match result {
                Ok(ServerResult::ListPromptsResult(page)) => page,
                Ok(_) => {
                    return Err(TransportError::Protocol(
                        "list prompts returned an unexpected response".to_string(),
                    ))
                }
                Err(error) => return Err(self.classify_client_failure(error).await),
            };
            prompts.extend(page.prompts);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        prompts
            .into_iter()
            .map(|prompt| {
                serde_json::to_value(prompt)
                    .and_then(serde_json::from_value)
                    .map_err(|err| {
                        TransportError::Protocol(format!("prompt serialization failed: {err}"))
                    })
            })
            .collect()
    }

    pub async fn get_prompt(
        &self,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let serde_json::Value::Object(args_map) = arguments else {
            return Err(TransportError::InvalidInput(
                "prompt arguments must be a JSON object".to_string(),
            ));
        };
        self.require_prompts()?;
        let request = GetPromptRequestParams::new(prompt_name).with_arguments(args_map);
        let result = send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::GetPromptRequest(GetPromptRequest::new(request)),
            "get prompt",
        )
        .await;
        let result = match result {
            Ok(ServerResult::GetPromptResult(result)) => result,
            Ok(_) => {
                return Err(TransportError::Protocol(
                    "get prompt returned an unexpected response".to_string(),
                ))
            }
            Err(error) => return Err(self.classify_client_failure(error).await),
        };
        serde_json::to_value(result).map_err(|err| {
            TransportError::Protocol(format!("prompt result serialization failed: {err}"))
        })
    }
}
