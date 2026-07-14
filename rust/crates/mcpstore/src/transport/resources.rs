use rmcp::model::ReadResourceRequestParams;

use crate::transport::client::McpConnection;
use crate::transport::{DiscoveredResource, DiscoveredResourceTemplate, Result, TransportError};

impl McpConnection {
    pub async fn list_resources(&self) -> Result<Vec<DiscoveredResource>> {
        let client = self.get_client()?;
        let resources = match client.list_all_resources().await {
            Ok(resources) => resources,
            Err(error) => {
                return Err(self
                    .classify_client_failure(TransportError::Protocol(format!(
                        "list_resources failed: {error}"
                    )))
                    .await);
            }
        };

        resources
            .into_iter()
            .map(|resource| {
                serde_json::to_value(resource)
                    .and_then(serde_json::from_value)
                    .map_err(|err| {
                        TransportError::Protocol(format!("resource serialization failed: {err}"))
                    })
            })
            .collect()
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<DiscoveredResourceTemplate>> {
        let client = self.get_client()?;
        let templates = match client.list_all_resource_templates().await {
            Ok(templates) => templates,
            Err(error) => {
                return Err(self
                    .classify_client_failure(TransportError::Protocol(format!(
                        "list_resource_templates failed: {error}"
                    )))
                    .await);
            }
        };

        templates
            .into_iter()
            .map(|template| {
                serde_json::to_value(template)
                    .and_then(serde_json::from_value)
                    .map_err(|err| {
                        TransportError::Protocol(format!(
                            "resource template serialization failed: {err}"
                        ))
                    })
            })
            .collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<serde_json::Value> {
        let client = self.get_client()?;
        let result = match client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
        {
            Ok(result) => result,
            Err(error) => {
                return Err(self
                    .classify_client_failure(TransportError::Protocol(format!(
                        "read_resource failed: {error}"
                    )))
                    .await);
            }
        };

        serde_json::to_value(result).map_err(|err| {
            TransportError::Protocol(format!("resource read serialization failed: {err}"))
        })
    }
}
