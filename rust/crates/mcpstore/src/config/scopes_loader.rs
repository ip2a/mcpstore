use crate::config::{ComponentOverride as ConfigComponentOverride, ScopesConfig};
use crate::overrides::{ToolOverridePatch, PromptOverridePatch, ResourceOverridePatch};
use crate::cache::models::ToolArgumentOverride;
use crate::store::prelude::*;

impl MCPStore {
    /// scopes.toml lives beside the resolved mcp.json: one data-dir authority,
    /// so a config-path override moves scopes.toml with the rest of the data.
    pub fn scopes_config_path(&self) -> Result<std::path::PathBuf> {
        self.config_manager()
            .mcp_path()
            .parent()
            .map(|dir| dir.join("scopes.toml"))
            .ok_or_else(|| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    "cannot resolve data directory from mcp.json path",
                )
            })
    }

    /// Load scopes.toml and populate openkeyv with overrides
    pub async fn load_scopes_config(&self) -> Result<()> {
        let path = self.scopes_config_path()?;
        // A broken scopes.toml fails loudly here; it must never silently
        // behave as an empty config.
        //
        // KNOWN LIMITATION (reconcile): loading is additive — sections removed
        // from scopes.toml stay live until the override is deleted via the
        // API. Prompt/resource overrides have no delete path yet, so a full
        // wipe-and-replay on reload is unsafe; revisit when delete APIs exist.
        let config = ScopesConfig::load_if_exists(&path)
            .map_err(|error| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    format!("failed to load {}: {error}", path.display()),
                )
            })?
            .unwrap_or_default();

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

        // Overrides and policy resolve against the instance's synced tool list,
        // which only exists after a first connect. Skipping a never-connected
        // instance keeps boot alive when scopes.toml references stopped
        // services; /config/reload after connecting reapplies everything. A
        // named tool missing from a connected instance stays a loud error.
        let instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await;
        if instance.as_ref().is_some_and(|instance| instance.tools.is_empty()) {
            tracing::info!(
                "Service '{}' in scope {:?} has no synced tools yet (not connected); \
                 skipping scopes.toml entries until connect + /config/reload",
                service_name,
                scope
            );
            return Ok(());
        }

        // Apply tool policy through the same enforcement path the panel API
        // uses (context tool visibility), not a separate store.
        if let Some(ref policy) = cfg.tools.policy {
            self.set_context_tool_visibility(instance_id, policy.clone())
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
}
