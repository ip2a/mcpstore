use crate::config::ServerConfig;
use crate::transport::content::content_item_from_rmcp;
use crate::transport::{http as http_transport, stdio as stdio_transport};
use crate::transport::{Result, ToolCallResult, ToolDescription, TransportError};

pub use crate::transport::pool::ConnectionPool;

use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};
use rmcp::service::{RoleClient, RunningService};

pub(super) type McpClient = RunningService<RoleClient, ()>;

enum ActiveClient {
    Stdio(McpClient),
    Http(McpClient),
}

pub struct McpConnection {
    name: String,
    config: ServerConfig,
    client: Option<ActiveClient>,
}

impl McpConnection {
    pub fn new(name: String, config: ServerConfig) -> Self {
        Self {
            name,
            config,
            client: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    pub async fn connect(&mut self) -> Result<()> {
        let transport_type = self.config.infer_transport();
        tracing::info!(
            "[TRANSPORT] Connecting to service {} (transport={})",
            self.name,
            transport_type
        );

        match transport_type {
            "stdio" => self.connect_stdio().await,
            "streamable-http" | "http" => self.connect_http().await,
            "sse" => self.connect_http().await,
            other => Err(TransportError::ConnectionFailed(format!(
                "Unsupported transport type: {other}"
            ))),
        }
    }

    async fn connect_stdio(&mut self) -> Result<()> {
        let client = stdio_transport::connect(&self.name, &self.config).await?;
        tracing::info!("[TRANSPORT] stdio connected: {}", self.name);
        self.client = Some(ActiveClient::Stdio(client));
        Ok(())
    }

    async fn connect_http(&mut self) -> Result<()> {
        let client = http_transport::connect(&self.name, &self.config).await?;
        tracing::info!("[TRANSPORT] HTTP connected: {}", self.name);
        self.client = Some(ActiveClient::Http(client));
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<ToolDescription>> {
        let client = self.get_client()?;
        let resp = client
            .list_tools(None)
            .await
            .map_err(|e| TransportError::Protocol(format!("list_tools failed: {e}")))?;

        let tools = resp
            .tools
            .into_iter()
            .map(|t| ToolDescription {
                name: t.name.to_string(),
                description: t.description.unwrap_or_default().to_string(),
                input_schema: serde_json::to_value(&t.input_schema).unwrap_or_default(),
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
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };

        let param = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args_map);

        let resp = client
            .call_tool(param)
            .await
            .map_err(|e| TransportError::ToolCallFailed(format!("{e}")))?;

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

    pub async fn list_resources(&self) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client()?;
        let resources = client
            .list_all_resources()
            .await
            .map_err(|e| TransportError::Protocol(format!("list_resources failed: {e}")))?;

        resources
            .into_iter()
            .map(|resource| {
                serde_json::to_value(resource).map_err(|e| {
                    TransportError::Protocol(format!("resource serialization failed: {e}"))
                })
            })
            .collect()
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client()?;
        let templates = client.list_all_resource_templates().await.map_err(|e| {
            TransportError::Protocol(format!("list_resource_templates failed: {e}"))
        })?;

        templates
            .into_iter()
            .map(|template| {
                serde_json::to_value(template).map_err(|e| {
                    TransportError::Protocol(format!("resource template serialization failed: {e}"))
                })
            })
            .collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<serde_json::Value> {
        let client = self.get_client()?;
        let result = client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
            .map_err(|e| TransportError::Protocol(format!("read_resource failed: {e}")))?;

        serde_json::to_value(result).map_err(|e| {
            TransportError::Protocol(format!("resource read serialization failed: {e}"))
        })
    }

    pub async fn list_prompts(&self) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client()?;
        let prompts = client
            .list_all_prompts()
            .await
            .map_err(|e| TransportError::Protocol(format!("list_prompts failed: {e}")))?;

        prompts
            .into_iter()
            .map(|prompt| {
                serde_json::to_value(prompt).map_err(|e| {
                    TransportError::Protocol(format!("prompt serialization failed: {e}"))
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
            .map_err(|e| TransportError::Protocol(format!("get_prompt failed: {e}")))?;

        serde_json::to_value(result).map_err(|e| {
            TransportError::Protocol(format!("prompt result serialization failed: {e}"))
        })
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(client) = self.client.take() {
            let inner = match client {
                ActiveClient::Stdio(c) => c,
                ActiveClient::Http(c) => c,
            };
            inner.cancel().await.ok();
            tracing::info!("[TRANSPORT] Disconnected: {}", self.name);
        }
        Ok(())
    }

    fn get_client(&self) -> Result<&McpClient> {
        match &self.client {
            Some(ActiveClient::Stdio(c)) => Ok(c),
            Some(ActiveClient::Http(c)) => Ok(c),
            None => Err(TransportError::NotConnected(self.name.clone())),
        }
    }
}
