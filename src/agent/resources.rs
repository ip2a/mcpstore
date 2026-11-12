use crate::agent::models::insert_instance_identity;
use crate::overrides::resources::apply_resource;
use crate::store::prelude::*;
impl MCPStore {
    pub async fn list_resources_scoped(&self, scope: &ScopeRef) -> Result<Vec<serde_json::Value>> {
        self.collect_scope_resources_scoped(scope).await
    }

    pub async fn list_resources_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<serde_json::Value>> {
        let instance = self.require_instance(instance_id).await?;
        let mut resources = self.list_resources(instance_id).await?;
        resources.sort_by(|left, right| left.uri.cmp(&right.uri));
        let uris: Vec<String> = resources
            .iter()
            .map(|resource| resource.uri.clone())
            .collect();
        // One batched read for the whole set — never one KV read per resource.
        let overrides = self
            .enabled_resource_overrides_for_uris(instance_id, &uris)
            .await?;
        let mut out = Vec::with_capacity(resources.len());
        for resource in resources {
            let Some(mut value) = apply_resource(overrides.get(&resource.uri), &resource)? else {
                continue;
            };
            insert_instance_identity(
                &mut value,
                instance_id,
                &instance.service_name,
                &instance.scope,
            );
            out.push(value);
        }
        Ok(out)
    }

    /// Scope-view resource catalog in one page. A failed instance
    /// (disconnected, auth pending) is skipped — counted in
    /// `skipped_instances`, never blanks the page; the per-instance route
    /// stays loud.
    pub async fn list_resources_viewed(
        &self,
        view: &ScopeView,
    ) -> Result<ScopedCatalogPage<serde_json::Value>> {
        let instances = self.list_view_instances(view).await?;
        let mut entries = Vec::new();
        let mut skipped_instances = 0usize;
        for instance in &instances {
            match self.list_resources_for_instance(instance.instance_id).await {
                Ok(instance_resources) => entries.extend(instance_resources),
                Err(error) => {
                    skipped_instances += 1;
                    tracing::warn!(
                        %error,
                        service = %instance.service_name,
                        "scope resource catalog skipped a failed instance"
                    );
                }
            }
        }
        entries.sort_by(|left, right| {
            left.get("service_name")
                .and_then(serde_json::Value::as_str)
                .cmp(
                    &right
                        .get("service_name")
                        .and_then(serde_json::Value::as_str),
                )
                .then_with(|| {
                    left.get("uri")
                        .and_then(serde_json::Value::as_str)
                        .cmp(&right.get("uri").and_then(serde_json::Value::as_str))
                })
        });
        Ok(ScopedCatalogPage {
            entries,
            skipped_instances,
        })
    }

    pub async fn list_resource_templates_scoped(
        &self,
        scope: &ScopeRef,
    ) -> Result<Vec<serde_json::Value>> {
        self.collect_scope_resource_templates_scoped(scope).await
    }

    pub async fn list_resource_templates_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Vec<serde_json::Value>> {
        let instance = self.require_instance(instance_id).await?;
        let mut templates = self.list_resource_templates(instance_id).await?;
        templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
        let keys: Vec<String> = templates
            .iter()
            .map(|template| template.uri_template.clone())
            .collect();
        // One batched read for the whole set — never one KV read per template.
        let overrides = self
            .enabled_resource_template_overrides_for_keys(instance_id, &keys)
            .await?;
        let mut out = Vec::with_capacity(templates.len());
        for template in templates {
            let Some(mut value) = apply_resource(overrides.get(&template.uri_template), &template)?
            else {
                continue;
            };
            insert_instance_identity(
                &mut value,
                instance_id,
                &instance.service_name,
                &instance.scope,
            );
            out.push(value);
        }
        Ok(out)
    }

    pub async fn read_resource_scoped(
        &self,
        instance_id: InstanceId,
        uri: &str,
    ) -> Result<serde_json::Value> {
        self.require_instance(instance_id).await?;
        // Execution surface: connect on demand BEFORE resolving the original
        // key — list_resources is side-effect free now and answers
        // NotConnected for a disconnected instance.
        self.ensure_instance_connected(instance_id).await?;
        let raw_uris: Vec<String> = self
            .list_resources(instance_id)
            .await?
            .into_iter()
            .map(|resource| resource.uri)
            .collect();
        let original = self
            .ensure_original_key_for_component(
                crate::overrides::ComponentKind::Resource,
                instance_id,
                uri,
                &raw_uris,
            )
            .await?;
        if self
            .load_resource_override(instance_id, &original)
            .await?
            .is_some_and(|rule| rule.common.enabled == Some(false))
        {
            return Err(crate::Error::new(
                FailureCode::Internal,
                format!("Resource '{uri}' is disabled in instance '{instance_id}'"),
            ));
        }
        self.read_resource(instance_id, &original).await
    }
}
