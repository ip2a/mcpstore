use std::collections::HashMap;

use crate::overrides::ServiceOverrideRule;
use crate::store::prelude::*;

impl MCPStore {
    pub async fn list_services(&self) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_instances().await;
        self.enrich_services(instances).await
    }

    pub async fn list_services_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        let instances = self.list_scope_instances(scope).await?;
        self.enrich_services(instances).await
    }

    /// One override map for the whole list — cards must not N+1 `/service_override`.
    async fn enrich_services(
        &self,
        instances: Vec<ServiceInstance>,
    ) -> Result<Vec<serde_json::Value>> {
        let overrides: HashMap<InstanceId, ServiceOverrideRule> = self
            .list_service_overrides_lenient()
            .await?
            .into_iter()
            .map(|rule| (rule.instance_id, rule))
            .collect();
        let mut services = Vec::with_capacity(instances.len());
        for instance in instances {
            let override_rule = overrides.get(&instance.instance_id);
            services.push(self.enrich_service(instance, override_rule).await?);
        }
        Ok(services)
    }

    async fn enrich_service(
        &self,
        instance: ServiceInstance,
        override_rule: Option<&ServiceOverrideRule>,
    ) -> Result<serde_json::Value> {
        let instance_id = instance.instance_id;
        let service_name = instance.service_name.clone();
        let tool_count = instance.tools.len();
        let config_description = instance
            .effective_config
            .get("description")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let state = self.service_state_entry(instance_id).await?;
        let mcp = self.mcp_server_metadata(instance_id).await?;
        let mut value = serde_json::to_value(&instance)
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        if let serde_json::Value::Object(object) = &mut value {
            object.insert("tool_count".to_string(), serde_json::json!(tool_count));
            object.insert(
                "state".to_string(),
                serde_json::to_value(state)
                    .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?,
            );
            // Same as service_info_scoped: list cards need mcp.serverInfo.icons.
            object.insert(
                "mcp".to_string(),
                serde_json::to_value(mcp)
                    .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?,
            );
            apply_list_projection(
                object,
                &service_name,
                config_description.as_deref(),
                override_rule,
            );
        }
        Ok(value)
    }

    pub async fn list_service_entries_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<ScopedServiceEntry>> {
        let instances = self.list_scope_instances(scope).await?;
        let mut services = Vec::with_capacity(instances.len());
        for instance in instances {
            let state = self.service_state_entry(instance.instance_id).await?;
            services.push(ScopedServiceEntry {
                tool_count: instance.tools.len(),
                instance,
                state,
            });
        }
        Ok(services)
    }

    pub async fn service_info_scoped(&self, instance_id: InstanceId) -> Result<serde_json::Value> {
        self.refresh_from_db_if_needed().await?;
        let instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await
            .ok_or_else(|| Error::new(FailureCode::ServiceNotFound, instance_id.to_string()))?;
        let override_rule = self.get_service_override(instance_id).await?;
        self.enrich_service(instance, override_rule.as_ref()).await
    }

    /// List services by read view: All aggregates all (`list_services`); Store/Agent pass through to `list_services_scoped`.
    pub async fn list_services_viewed(&self, view: &ScopeView) -> Result<Vec<serde_json::Value>> {
        match view {
            ScopeView::All => self.list_services().await,
            ScopeView::Store => self.list_services_scoped(&ScopeRef::Store).await,
            ScopeView::Agent { agent_id } => {
                self.list_services_scoped(&ScopeRef::Agent {
                    agent_id: agent_id.clone(),
                })
                .await
            }
        }
    }
}

/// Card contract fields: always present so the list UI never fans out to `/service_override`.
fn apply_list_projection(
    object: &mut serde_json::Map<String, serde_json::Value>,
    service_name: &str,
    config_description: Option<&str>,
    override_rule: Option<&ServiceOverrideRule>,
) {
    let display_name = override_rule
        .and_then(|rule| rule.common.display_name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(service_name);
    object.insert(
        "display_name".to_string(),
        serde_json::Value::String(display_name.to_string()),
    );

    let description = override_rule
        .and_then(|rule| rule.common.description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(config_description)
        .unwrap_or("");
    object.insert(
        "description".to_string(),
        serde_json::Value::String(description.to_string()),
    );

    if let Some(icons) = override_rule.and_then(|rule| rule.common.icons.clone()) {
        object.insert("icons".to_string(), icons);
    }
}
