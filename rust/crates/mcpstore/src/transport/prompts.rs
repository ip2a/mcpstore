use rmcp::model::GetPromptRequestParams;

use crate::transport::client::McpConnection;
use crate::transport::{DiscoveredPrompt, Result, TransportError};

impl McpConnection {
    pub async fn list_prompts(&self) -> Result<Vec<DiscoveredPrompt>> {
        let client = self.get_client()?;
        let prompts = client
            .list_all_prompts()
            .await
            .map_err(|err| TransportError::Protocol(format!("list_prompts failed: {err}")))?;

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
        let client = self.get_client()?;
        let args_map = match arguments {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let request = GetPromptRequestParams::new(prompt_name).with_arguments(args_map);
        let result = client
            .get_prompt(request)
            .await
            .map_err(|err| TransportError::Protocol(format!("get_prompt failed: {err}")))?;

        serde_json::to_value(result).map_err(|err| {
            TransportError::Protocol(format!("prompt result serialization failed: {err}"))
        })
    }
}
