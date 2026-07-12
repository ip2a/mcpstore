use crate::store::prelude::*;

impl MCPStore {
    pub(crate) async fn collect_store_resources_scoped(&self) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(None).await?;
        targets.sort();

        let mut resources = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_resources = self.list_resources(&global_service_name).await?;
            service_resources.sort_by(|left, right| left.uri.cmp(&right.uri));
            for resource in service_resources {
                resources.push(Self::resource_payload_value(
                    resource,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(resources)
    }

    pub(crate) async fn collect_agent_resources_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(Some(agent_id)).await?;
        targets.sort();

        let mut resources = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_resources = self.list_resources(&global_service_name).await?;
            service_resources.sort_by(|left, right| left.uri.cmp(&right.uri));
            for resource in service_resources {
                resources.push(Self::resource_payload_value(
                    resource,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(resources)
    }

    pub(crate) async fn collect_store_resource_templates_scoped(
        &self,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(None).await?;
        targets.sort();

        let mut templates = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_templates = self.list_resource_templates(&global_service_name).await?;
            service_templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
            for template in service_templates {
                templates.push(Self::resource_template_payload_value(
                    template,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(templates)
    }

    pub(crate) async fn collect_agent_resource_templates_scoped(
        &self,
        agent_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let mut targets = self.scoped_service_bindings(Some(agent_id)).await?;
        targets.sort();

        let mut templates = Vec::new();
        for (display_service_name, global_service_name) in targets {
            let mut service_templates = self.list_resource_templates(&global_service_name).await?;
            service_templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
            for template in service_templates {
                templates.push(Self::resource_template_payload_value(
                    template,
                    display_service_name.clone(),
                    global_service_name.clone(),
                )?);
            }
        }
        Ok(templates)
    }
}
