use crate::config::{ComponentOverride as ConfigComponentOverride, ScopesConfig};
use crate::overrides::{ToolOverridePatch, PromptOverridePatch, ResourceOverridePatch};
use crate::cache::models::ToolArgumentOverride;
use crate::store::prelude::*;

impl MCPStore {
    /// Load scopes.toml and populate openkeyv with overrides
    pub async fn load_scopes_config(&self) -> Result<()> {
        let config = ScopesConfig::load_or_default();

        // Load store scope
        for (service_name, service_cfg) in &config.store.services {
            self.apply_service_scope_config(service_name, &ScopeRef::Store, service_cfg)
                .await?;
        }

        // Load agent scopes
        for (agent_id, agent_cfg) in &config.agents {
            for (service_name, service_cfg) in &agent_cfg.services {
                let scope = ScopeRef::Agent { agent_id: agent_id.clone() };
                self.apply_service_scope_config(service_name, &scope, service_cfg)
                    .await?;
            }
        }

        Ok(())
    }

    async fn apply_service_scope_config(
        &self,
        service_name: &str,
        scope: &ScopeRef,
        cfg: &crate::config::ServiceScopeConfig,
    ) -> Result<()> {
        // Resolve instance_id for this service in this scope
        let instance_id = match self.instance_id_for_scope(service_name, scope).await {
            Ok(id) => id,
            Err(_) => {
                tracing::warn!(
                    "Service '{}' in scope {:?} not found, skipping config",
                    service_name,
                    scope
                );
                return Ok(());
            }
        };

        // Apply tool policy (if specified)
        if let Some(ref policy) = cfg.tools.policy {
            self.set_instance_tool_policy_internal(instance_id, policy.clone())
                .await?;
        }

        // Apply tool overrides
        for (tool_name, override_cfg) in &cfg.tools.overrides {
            let patch = self.config_override_to_tool_patch(override_cfg);
            self.set_tool_override(instance_id, tool_name, patch)
                .await?;
        }

        // Apply prompt overrides
        for (prompt_name, override_cfg) in &cfg.prompts.overrides {
            let patch = self.config_override_to_prompt_patch(override_cfg);
            self.set_prompt_override(instance_id, prompt_name, patch)
                .await?;
        }

        // Apply resource overrides
        for (resource_uri, override_cfg) in &cfg.resources.overrides {
            let patch = self.config_override_to_resource_patch(override_cfg);
            self.set_resource_override(instance_id, resource_uri, patch)
                .await?;
        }

        Ok(())
    }

    fn config_override_to_tool_patch(&self, cfg: &ConfigComponentOverride) -> ToolOverridePatch {
        let mut patch = ToolOverridePatch::default();

        if let Some(ref name) = cfg.display_name {
            patch = patch.with_display_name(name);
        }
        if let Some(ref desc) = cfg.description {
            patch = patch.with_description(desc);
        }
        if let Some(enabled) = cfg.enabled {
            patch = patch.enabled(enabled);
        }
        if !cfg.tags.is_empty() {
            patch.common.tags = cfg.tags.clone();
        }
        if let Some(ref meta) = cfg.meta {
            patch.common.meta = Some(meta.clone());
        }
        if let Some(ref annotations) = cfg.annotations {
            patch.common.annotations = Some(annotations.clone());
        }
        if let Some(ref icons) = cfg.icons {
            patch.common.icons = Some(serde_json::json!({
                "light": icons.light,
                "dark": icons.dark,
            }));
        }
        if let Some(ref args) = cfg.arguments {
            for (arg_name, arg_override) in args {
                if let Some(ref default) = arg_override.default {
                    patch = patch.with_argument(ToolArgumentOverride {
                        original_name: arg_name.clone(),
                        new_name: None,
                        hidden: false,
                        default_value: Some(default.clone()),
                        description: None,
                        validation_schema: None,
                    });
                }
            }
        }

        patch
    }

    fn config_override_to_prompt_patch(&self, cfg: &ConfigComponentOverride) -> PromptOverridePatch {
        let mut patch = PromptOverridePatch::default();

        if let Some(ref name) = cfg.display_name {
            patch = patch.with_display_name(name);
        }
        if let Some(ref desc) = cfg.description {
            patch = patch.with_description(desc);
        }
        if let Some(enabled) = cfg.enabled {
            patch.common.enabled = Some(enabled);
        }
        if !cfg.tags.is_empty() {
            patch.common.tags = cfg.tags.clone();
        }
        if let Some(ref meta) = cfg.meta {
            patch.common.meta = Some(meta.clone());
        }
        if let Some(ref annotations) = cfg.annotations {
            patch.common.annotations = Some(annotations.clone());
        }

        patch
    }

    fn config_override_to_resource_patch(&self, cfg: &ConfigComponentOverride) -> ResourceOverridePatch {
        let mut patch = ResourceOverridePatch::default();

        if let Some(ref name) = cfg.display_name {
            patch = patch.with_display_name(name);
        }
        if let Some(ref desc) = cfg.description {
            patch = patch.with_description(desc);
        }
        if let Some(enabled) = cfg.enabled {
            patch.common.enabled = Some(enabled);
        }
        if !cfg.tags.is_empty() {
            patch.common.tags = cfg.tags.clone();
        }
        if let Some(ref meta) = cfg.meta {
            patch.common.meta = Some(meta.clone());
        }
        if let Some(ref annotations) = cfg.annotations {
            patch.common.annotations = Some(annotations.clone());
        }

        patch
    }

    async fn set_instance_tool_policy_internal(&self, instance_id: InstanceId, policy: Vec<String>) -> Result<()> {
        // Store tool policy in openkeyv
        // Assuming there's a set_instance_tool_policy method or similar
        // If not, we need to add it or use existing set_tool_policy APIs
        let key = format!("tool_policy::{}", instance_id);
        self.kernel.persistence.cache.put_state(
            "tool_policies",
            &key,
            serde_json::json!(policy),
        ).await.map_err(Error::from)
    }
}
