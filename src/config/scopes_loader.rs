use crate::config::{ComponentOverride as ConfigComponentOverride, ScopesConfig};
use crate::overrides::{
    PromptOverridePatch, ResourceOverridePatch, ServiceOverridePatch, ToolOverridePatch,
};
use crate::cache::models::ToolArgumentOverride;
use crate::store::prelude::*;

const APPLIED_JOURNAL_STATE_TYPE: &str = "scopes_loader";
const APPLIED_JOURNAL_KEY: &str = "applied";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AppliedJournal {
    #[serde(default)]
    entries: Vec<(String, String)>,
}

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

    /// Load scopes.toml and populate openkeyv with overrides.
    ///
    /// A broken scopes.toml fails loudly here; it must never silently behave
    /// as an empty config.
    ///
    /// Reconcile: every override this loader writes is journaled by its exact
    /// state key. On the next load, journaled entries the file no longer
    /// declares are deleted — removing a section from scopes.toml actually
    /// removes the override on reload or restart. Deletes are raw KV removals
    /// (no registry lookups), so entries for uninstalled services clean up
    /// too. Panel-made overrides never enter the journal and are never
    /// touched by reconciliation.
    pub async fn load_scopes_config(&self) -> Result<()> {
        let path = self.scopes_config_path()?;
        let config = ScopesConfig::load_if_exists(&path)
            .map_err(|error| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    format!("failed to load {}: {error}", path.display()),
                )
            })?
            .unwrap_or_default();

        // put_state only accepts object values, so the journal is wrapped;
        // a malformed/legacy shape degrades to "nothing previously applied".
        let previous: Vec<(String, String)> = self
            .kernel
            .persistence
            .cache
            .get_state(APPLIED_JOURNAL_STATE_TYPE, APPLIED_JOURNAL_KEY)
            .await?
            .and_then(|value| serde_json::from_value::<AppliedJournal>(value).ok())
            .map(|journal| journal.entries)
            .unwrap_or_default();

        let mut applied: Vec<(String, String)> = Vec::new();
        // Load store scope
        for (service_name, service_cfg) in &config.store.services {
            self.apply_service_scope_config(service_name, &ScopeRef::Store, service_cfg, &mut applied)
                .await?;
        }

        // Load agent scopes
        for (agent_id, agent_cfg) in &config.agents {
            for (service_name, service_cfg) in &agent_cfg.services {
                let scope = ScopeRef::Agent { agent_id: agent_id.clone() };
                self.apply_service_scope_config(service_name, &scope, service_cfg, &mut applied)
                    .await?;
            }
        }

        for entry in &previous {
            if !applied.contains(entry) {
                if let Err(error) = self
                    .kernel
                    .persistence
                    .cache
                    .delete_state(&entry.0, &entry.1)
                    .await
                {
                    tracing::warn!("failed to delete stale scopes.toml override {entry:?}: {error}");
                }
            }
        }
        self.kernel
            .persistence
            .cache
            .put_state(
                APPLIED_JOURNAL_STATE_TYPE,
                APPLIED_JOURNAL_KEY,
                serde_json::json!({ "entries": applied }),
            )
            .await?;
        Ok(())
    }

    async fn apply_service_scope_config(
        &self,
        service_name: &str,
        scope: &ScopeRef,
        cfg: &crate::config::ServiceScopeConfig,
        applied: &mut Vec<(String, String)>,
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

        // Service-level projection (rename/description/hide) never resolves
        // against the synced tool list, so it applies even before a first
        // connect.
        {
            let mut common = crate::overrides::ComponentOverrideCommon::default();
            common.display_name = cfg.display_name.clone();
            common.description = cfg.description.clone();
            common.enabled = cfg.enabled;
            common.tags = cfg.tags.clone();
            common.meta = cfg.meta.clone();
            common.annotations = cfg.annotations.clone();
            common.icons = service_icons_json(cfg.icons.as_ref());
            if common != crate::overrides::ComponentOverrideCommon::default() {
                let patch = ServiceOverridePatch { common };
                self.set_service_override(instance_id, patch).await?;
                applied.push((
                    crate::overrides::services::SERVICE_OVERRIDES_STATE_TYPE.to_string(),
                    instance_id.to_string(),
                ));
            }
        }

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
            let context_key = Self::build_tool_visibility_context_key(scope)?;
            applied.push((
                crate::agent::tool_visibility::CONTEXT_TOOL_VISIBILITY_STATE_TYPE.to_string(),
                format!("{context_key}:{instance_id}"),
            ));
        }

        // Apply tool overrides
        for (tool_name, override_cfg) in &cfg.tools.overrides {
            let patch = self.config_override_to_tool_patch(override_cfg);
            let rule = self.set_tool_override(instance_id, tool_name, patch).await?;
            applied.push((
                crate::overrides::tools::TOOL_OVERRIDES_STATE_TYPE.to_string(),
                Self::component_override_key(rule.instance_id, &rule.tool_name),
            ));
        }

        // Apply prompt overrides
        for (prompt_name, override_cfg) in &cfg.prompts.overrides {
            let patch = self.config_override_to_prompt_patch(override_cfg);
            let rule = self
                .set_prompt_override(instance_id, prompt_name, patch)
                .await?;
            applied.push((
                crate::overrides::prompts::PROMPT_OVERRIDES_STATE_TYPE.to_string(),
                Self::component_override_key(rule.instance_id, &rule.prompt_name),
            ));
        }

        // Apply resource overrides
        for (resource_uri, override_cfg) in &cfg.resources.overrides {
            let patch = self.config_override_to_resource_patch(override_cfg);
            let rule = self
                .set_resource_override(instance_id, resource_uri, patch)
                .await?;
            applied.push((
                crate::overrides::resources::RESOURCE_OVERRIDES_STATE_TYPE.to_string(),
                Self::component_override_key(rule.instance_id, &rule.uri),
            ));
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

/// scopes.toml `icons = { light = …, dark = … }` → the JSON shape the
/// override rules carry. `None` when neither key is present.
fn service_icons_json(icons: Option<&crate::config::IconsOverride>) -> Option<serde_json::Value> {
    let icons = icons?;
    let mut map = serde_json::Map::new();
    if let Some(light) = &icons.light {
        map.insert("light".to_string(), serde_json::Value::String(light.clone()));
    }
    if let Some(dark) = &icons.dark {
        map.insert("dark".to_string(), serde_json::Value::String(dark.clone()));
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The journal lives behind put_state, which rejects non-object values.
    /// Pin the object shape so a future edit can't silently flip it to a bare
    /// array and break every reconcile (reads would degrade to empty).
    #[test]
    fn journal_round_trips_through_object_value() {
        let journal = AppliedJournal {
            entries: vec![
                ("tool_overrides".to_string(), "inst-1:orig_tool".to_string()),
                (
                    "context_tool_visibility".to_string(),
                    "store:inst-1".to_string(),
                ),
            ],
        };
        let value = serde_json::to_value(&journal).unwrap();
        assert!(value.is_object(), "put_state rejects non-object values");
        let restored: AppliedJournal = serde_json::from_value(value).unwrap();
        assert_eq!(restored.entries, journal.entries);
        // Missing entries field (hand-edited file) degrades to empty, not error.
        let empty: AppliedJournal = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.entries.is_empty());
    }
}
