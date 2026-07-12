use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_resources_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        match (agent_id, service_name) {
            (_, Some(service_name)) => {
                let (display_service_name, global_service_name) = self
                    .resolve_scoped_service_binding(agent_id, service_name)
                    .await?;
                let mut resources = self.list_resources(&global_service_name).await?;
                resources.sort_by(|left, right| left.uri.cmp(&right.uri));
                resources
                    .into_iter()
                    .map(|resource| {
                        Self::resource_payload_value(
                            resource,
                            display_service_name.clone(),
                            global_service_name.clone(),
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_resources_scoped().await,
            (Some(agent_id), None) => self.collect_agent_resources_scoped(agent_id).await,
        }
    }

    pub async fn list_resource_templates_scoped(
        &self,
        agent_id: Option<&str>,
        service_name: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        match (agent_id, service_name) {
            (_, Some(service_name)) => {
                let (display_service_name, global_service_name) = self
                    .resolve_scoped_service_binding(agent_id, service_name)
                    .await?;
                let mut templates = self.list_resource_templates(&global_service_name).await?;
                templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
                templates
                    .into_iter()
                    .map(|template| {
                        Self::resource_template_payload_value(
                            template,
                            display_service_name.clone(),
                            global_service_name.clone(),
                        )
                    })
                    .collect()
            }
            (None, None) => self.collect_store_resource_templates_scoped().await,
            (Some(agent_id), None) => self.collect_agent_resource_templates_scoped(agent_id).await,
        }
    }

    pub async fn read_resource_scoped(
        &self,
        agent_id: Option<&str>,
        uri: &str,
        service_name: Option<&str>,
    ) -> Result<serde_json::Value> {
        let (_, global_service_name) = self
            .resolve_resource_service_binding(agent_id, uri, service_name)
            .await?;
        self.read_resource(&global_service_name, uri).await
    }
}
