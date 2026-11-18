use rmcp::model::{
    ClientRequest, ListResourceTemplatesRequest, ListResourcesRequest, PaginatedRequestParams,
    ReadResourceRequest, ReadResourceRequestParams, ServerResult,
};

use crate::error::Result;
use crate::error::{Error, FailureCode};
use crate::transport::client::McpConnection;
use crate::transport::protocol::send_protocol_request;
use crate::transport::{DiscoveredResource, DiscoveredResourceTemplate};

impl McpConnection {
    pub async fn list_resources(&self) -> Result<Vec<DiscoveredResource>> {
        self.require_resources()?;
        let mut resources = Vec::new();
        let mut cursor = None;
        loop {
            let request = ListResourcesRequest::with_param(
                PaginatedRequestParams::default().with_cursor(cursor),
            );
            let result = send_protocol_request(
                self.get_client()?,
                self.instance_id(),
                ClientRequest::ListResourcesRequest(request),
                "list resources",
            )
            .await;
            let page = match result {
                Ok(ServerResult::ListResourcesResult(page)) => page,
                Ok(_) => {
                    return Err(Error::new(
                        FailureCode::ToolFailed,
                        "list resources returned an unexpected response",
                    ))
                }
                Err(error) => return Err(self.classify_client_failure(error).await),
            };
            resources.extend(page.resources);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        resources
            .into_iter()
            .map(|resource| {
                serde_json::to_value(resource)
                    .and_then(serde_json::from_value)
                    .map_err(|err| {
                        Error::new(
                            FailureCode::ToolFailed,
                            format!("resource serialization failed: {err}"),
                        )
                    })
            })
            .collect()
    }

    pub async fn list_resource_templates(&self) -> Result<Vec<DiscoveredResourceTemplate>> {
        self.require_resources()?;
        let mut templates = Vec::new();
        let mut cursor = None;
        loop {
            let request = ListResourceTemplatesRequest::with_param(
                PaginatedRequestParams::default().with_cursor(cursor),
            );
            let result = send_protocol_request(
                self.get_client()?,
                self.instance_id(),
                ClientRequest::ListResourceTemplatesRequest(request),
                "list resource templates",
            )
            .await;
            let page = match result {
                Ok(ServerResult::ListResourceTemplatesResult(page)) => page,
                Ok(_) => {
                    return Err(Error::new(
                        FailureCode::ToolFailed,
                        "list resource templates returned an unexpected response",
                    ))
                }
                Err(error) => return Err(self.classify_client_failure(error).await),
            };
            templates.extend(page.resource_templates);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        templates
            .into_iter()
            .map(|template| {
                serde_json::to_value(template)
                    .and_then(serde_json::from_value)
                    .map_err(|err| {
                        Error::new(
                            FailureCode::ToolFailed,
                            format!("resource template serialization failed: {err}"),
                        )
                    })
            })
            .collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<serde_json::Value> {
        self.read_resource_traced(uri, None).await
    }

    pub(crate) async fn read_resource_traced(
        &self,
        uri: &str,
        trace: Option<std::sync::Arc<crate::telemetry::CallTrace>>,
    ) -> Result<serde_json::Value> {
        self.require_resources()?;
        let mut params = ReadResourceRequestParams::new(uri);
        if let Some(trace) = trace {
            params
                .meta
                .get_or_insert_default()
                .insert(
                    crate::telemetry::CALL_ID_META_KEY.to_string(),
                    trace.call_id().to_string().into(),
                );
        }
        let result = send_protocol_request(
            self.get_client()?,
            self.instance_id(),
            ClientRequest::ReadResourceRequest(ReadResourceRequest::new(params)),
            "read resource",
        )
        .await;
        let result = match result {
            Ok(ServerResult::ReadResourceResult(result)) => result,
            Ok(_) => {
                return Err(Error::new(
                    FailureCode::ToolFailed,
                    "read resource returned an unexpected response",
                ))
            }
            Err(error) => return Err(self.classify_client_failure(error).await),
        };
        serde_json::to_value(result).map_err(|err| {
            Error::new(
                FailureCode::ToolFailed,
                format!("resource read serialization failed: {err}"),
            )
        })
    }
}
