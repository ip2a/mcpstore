use rmcp::model::ReadResourceRequestParams;

use crate::transport::client::McpConnection;
use crate::transport::{Result, TransportError};

impl McpConnection {
    pub async fn list_resources(&self) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client()?;
        let resources = client
            .list_all_resources()
            .await
            .map_err(|err| TransportError::Protocol(format!("list_resources failed: {err}")))?;

        resources
            .into_iter()
            .map(|resource| {
                serde_json::to_value(resource).map_err(|err| {
                    TransportError::Protocol(format!("resource serialization failed: {err}"))
                })
            })
            .collect()
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client()?;
        let templates = client.list_all_resource_templates().await.map_err(|err| {
            TransportError::Protocol(format!("list_resource_templates failed: {err}"))
        })?;

        templates
            .into_iter()
            .map(|template| {
                serde_json::to_value(template).map_err(|err| {
                    TransportError::Protocol(format!(
                        "resource template serialization failed: {err}"
                    ))
                })
            })
            .collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<serde_json::Value> {
        let client = self.get_client()?;
        let result = client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
            .map_err(|err| TransportError::Protocol(format!("read_resource failed: {err}")))?;

        serde_json::to_value(result).map_err(|err| {
            TransportError::Protocol(format!("resource read serialization failed: {err}"))
        })
    }
}
