use crate::config::{ComponentRoute as ConfigComponentRoute, RouteConfig};
use crate::kv::models::ToolArgumentRoute;
use crate::routing::{
    PromptRoutePatch, ResourceRoutePatch, ServiceRoutePatch, ToolRoutePatch,
};
use crate::store::prelude::*;

const APPLIED_JOURNAL_STATE_TYPE: &str = "route_loader";
const APPLIED_JOURNAL_KEY: &str = "applied";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AppliedJournal {
    #[serde(default)]
    entries: Vec<(String, String)>,
}

impl MCPStore {
    /// route.toml lives beside the resolved mcp.json: one data-dir authority,
    /// so a config-path override moves route.toml with the rest of the data.
    pub fn route_config_path(&self) -> Result<std::path::PathBuf> {
        self.config_manager()
            .mcp_path()
            .parent()
            .map(|dir| dir.join("route.toml"))
            .ok_or_else(|| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    "cannot resolve data directory from mcp.json path",
                )
            })
    }

    /// Resolve `route.toml`. If it is missing and a same-schema file formerly
    /// written as `scopes.toml` is present, copy once so existing rules are not
    /// dropped. This is a data-dir rename, not an API shim.
    pub fn ensure_route_toml_migrated(&self) -> Result<std::path::PathBuf> {
        let route_path = self.route_config_path()?;
        if route_path.exists() {
            return Ok(route_path);
        }
        let former_path = self
            .config_manager()
            .mcp_path()
            .parent()
            .map(|dir| dir.join("scopes.toml"))
            .ok_or_else(|| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    "cannot resolve data directory from mcp.json path",
                )
            })?;
        if former_path.exists() {
            if let Some(parent) = route_path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    Error::new(
                        FailureCode::ConfigInvalid,
                        format!("failed to create data dir for route.toml: {error}"),
                    )
                })?;
            }
            std::fs::copy(&former_path, &route_path).map_err(|error| {
                Error::new(
                    FailureCode::ConfigInvalid,
                    format!(
                        "failed to move {} → {}: {error}",
                        former_path.display(),
                        route_path.display()
                    ),
                )
            })?;
            tracing::info!(
                "Moved gateway route rules from {} to {}",
                former_path.display(),
                route_path.display()
            );
        }
        Ok(route_path)
    }

    /// One-shot KV rename: `*_overrides` state types → `*_routes`.
    async fn migrate_legacy_route_kv_types(&self) -> Result<()> {
        const PAIRS: &[(&str, &str)] = &[
            ("tool_overrides", "tool_routes"),
            ("prompt_overrides", "prompt_routes"),
            ("resource_overrides", "resource_routes"),
            ("resource_template_overrides", "resource_template_routes"),
            ("service_overrides", "service_routes"),
        ];
        for &(old_type, new_type) in PAIRS {
            let entries = self
                .kernel
                .persistence
                .kv
                .get_all_states_async(old_type)
                .await?;
            if entries.is_empty() {
                continue;
            }
            for (key, value) in entries {
                if self
                    .kernel
                    .persistence
                    .kv
                    .get_state(new_type, &key)
                    .await?
                    .is_none()
                {
                    self.kernel
                        .persistence
                        .kv
                        .put_state(new_type, &key, value)
                        .await?;
                }
                let _ = self
                    .kernel
                    .persistence
                    .kv
                    .delete_state(old_type, &key)
                    .await;
            }
            tracing::info!("Migrated KV state type {old_type} → {new_type}");
        }
        Ok(())
    }

    /// Load route.toml and populate openkeyv with gateway route rules.
    ///
    /// A broken route.toml fails loudly here; it must never silently behave
    /// as an empty config.
    ///
    /// Reconcile: every rule this loader writes is journaled by its exact
    /// state key. On the next load, journaled entries the file no longer
    /// declares are deleted — removing a section from route.toml actually
    /// removes the rule on reload or restart. Deletes are raw KV removals
    /// (no registry lookups), so entries for uninstalled services clean up
    /// too. Panel-made rules never enter the journal and are never
    /// touched by reconciliation.
    ///
    pub async fn load_route_config(&self) -> Result<()> {
        // Connect-completion replays this concurrently with API reloads; the
        // journal is read-modify-write, so serialize the whole load.
        static ROUTE_LOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = ROUTE_LOAD_LOCK.lock().await;
        let path = self.ensure_route_toml_migrated()?;
        self.migrate_legacy_route_kv_types().await?;
        let config = RouteConfig::load_if_exists(&path)
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
            .kv
            .get_state(APPLIED_JOURNAL_STATE_TYPE, APPLIED_JOURNAL_KEY)
            .await?
            .and_then(|value| serde_json::from_value::<AppliedJournal>(value).ok())
            .map(|journal| journal.entries)
            .unwrap_or_default();

        let mut applied: Vec<(String, String)> = Vec::new();
        // Load store scope
        for (service_name, service_cfg) in &config.store.services {
            self.apply_service_scope_config(
                service_name,
                &ScopeRef::Store,
                service_cfg,
                &mut applied,
            )
            .await?;
        }

        // Load agent scopes
        for (agent_id, agent_cfg) in &config.agents {
            for (service_name, service_cfg) in &agent_cfg.services {
                let scope = ScopeRef::Agent {
                    agent_id: agent_id.clone(),
                };
                self.apply_service_scope_config(service_name, &scope, service_cfg, &mut applied)
                    .await?;
            }
        }

        for entry in &previous {
            if !applied.contains(entry) {
                if let Err(error) = self
                    .kernel
                    .persistence
                    .kv
                    .delete_state(&entry.0, &entry.1)
                    .await
                {
                    tracing::warn!(
                        "failed to delete stale route.toml rule {entry:?}: {error}"
                    );
                }
            }
        }
        self.kernel
            .persistence
            .kv
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
        cfg: &crate::config::ServiceRouteConfig,
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
            let mut common = crate::routing::RouteFields::default();
            common.display_name = cfg.display_name.clone();
            common.description = cfg.description.clone();
            common.enabled = cfg.enabled;
            common.tags = cfg.tags.clone();
            common.meta = cfg.meta.clone();
            common.annotations = cfg.annotations.clone();
            common.icons = service_icons_json(cfg.icons.as_ref());
            if common != crate::routing::RouteFields::default() {
                let patch = ServiceRoutePatch { common };
                self.set_service_route(instance_id, patch).await?;
                applied.push((
                    crate::routing::services::SERVICE_ROUTES_STATE_TYPE.to_string(),
                    instance_id.to_string(),
                ));
            }
        }

        // Overrides and policy resolve against the instance's synced tool list,
        // which only exists after a first connect. Skipping a never-connected
        // instance keeps boot alive when route.toml references stopped
        // services; /config/reload after connecting reapplies everything. A
        // named tool missing from a connected instance stays a loud error.
        let instance = self
            .kernel
            .control
            .registry
            .find_instance(instance_id)
            .await;
        if instance
            .as_ref()
            .is_some_and(|instance| instance.tools.is_empty())
        {
            tracing::info!(
                "Service '{}' in scope {:?} has no synced tools yet (not connected); \
                 skipping route.toml entries until connect + /config/reload",
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

        // Apply tool routes
        for (tool_name, override_cfg) in &cfg.tools.routes {
            let patch = self.config_to_tool_route_patch(override_cfg);
            let rule = self
                .set_tool_route(instance_id, tool_name, patch)
                .await?;
            applied.push((
                crate::routing::tools::TOOL_ROUTES_STATE_TYPE.to_string(),
                Self::component_route_key(rule.instance_id, &rule.tool_name),
            ));
        }

        // Apply prompt routes
        for (prompt_name, override_cfg) in &cfg.prompts.routes {
            let patch = self.config_to_prompt_route_patch(override_cfg);
            let rule = self
                .set_prompt_route(instance_id, prompt_name, patch)
                .await?;
            applied.push((
                crate::routing::prompts::PROMPT_ROUTES_STATE_TYPE.to_string(),
                Self::component_route_key(rule.instance_id, &rule.prompt_name),
            ));
        }

        // Apply resource routes
        for (resource_uri, override_cfg) in &cfg.resources.routes {
            let patch = self.config_to_resource_route_patch(override_cfg);
            let rule = self
                .set_resource_route(instance_id, resource_uri, patch)
                .await?;
            applied.push((
                crate::routing::resources::RESOURCE_ROUTES_STATE_TYPE.to_string(),
                Self::component_route_key(rule.instance_id, &rule.uri),
            ));
        }

        Ok(())
    }

    fn config_to_tool_route_patch(&self, cfg: &ConfigComponentRoute) -> ToolRoutePatch {
        let mut patch = ToolRoutePatch::default();

        if let Some(ref name) = cfg.display_name {
            patch = patch.with_display_name(name);
        }
        if let Some(ref desc) = cfg.description {
            patch = patch.with_description(desc);
        }
        if let Some(enabled) = cfg.enabled {
            patch = patch.enabled(enabled);
        }
        if cfg.weight != 0 {
            patch = patch.with_weight(cfg.weight);
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
                let has_fields = arg_override.new_name.is_some()
                    || arg_override.hidden
                    || arg_override.default.is_some()
                    || arg_override.description.is_some()
                    || arg_override.validation_schema.is_some();
                if !has_fields {
                    continue;
                }
                patch = patch.with_argument(ToolArgumentRoute {
                    original_name: arg_name.clone(),
                    new_name: arg_override.new_name.clone(),
                    hidden: arg_override.hidden,
                    default_value: arg_override.default.clone(),
                    description: arg_override.description.clone(),
                    validation_schema: arg_override.validation_schema.clone(),
                });
            }
        }

        patch
    }

    fn config_to_prompt_route_patch(
        &self,
        cfg: &ConfigComponentRoute,
    ) -> PromptRoutePatch {
        let mut patch = PromptRoutePatch::default();

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

    fn config_to_resource_route_patch(
        &self,
        cfg: &ConfigComponentRoute,
    ) -> ResourceRoutePatch {
        let mut patch = ResourceRoutePatch::default();

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

/// route.toml `icons = { light = …, dark = … }` → the JSON shape the
/// route rules carry. `None` when neither key is present.
fn service_icons_json(icons: Option<&crate::config::RouteIcons>) -> Option<serde_json::Value> {
    let icons = icons?;
    let mut map = serde_json::Map::new();
    if let Some(light) = &icons.light {
        map.insert(
            "light".to_string(),
            serde_json::Value::String(light.clone()),
        );
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
                ("tool_routes".to_string(), "inst-1:orig_tool".to_string()),
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

    #[test]
    fn migrates_legacy_scopes_toml_once_into_route_toml() {
        use crate::store::{JsonStoreConfig, PanelRole, StoreOptions};

        let dir = std::env::temp_dir().join(format!(
            "mcpstore-route-migrate-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mcp_path = dir.join("mcp.json");
        std::fs::write(&mcp_path, "{}").unwrap();
        let scopes_path = dir.join("scopes.toml");
        let route_path = dir.join("route.toml");
        let legacy = r#"[store.services.weather.tools.routes.get_weather]
display_name = "Weather"
"#;
        std::fs::write(&scopes_path, legacy).unwrap();
        assert!(!route_path.exists());

        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(mcp_path.to_string_lossy().to_string()),
            store: Some(JsonStoreConfig::memory()),
            namespace: Some(format!("route-migrate-{}", uuid::Uuid::new_v4())),
            panel: PanelRole::ControlPanel,
            ..StoreOptions::default()
        })
        .unwrap();

        let resolved = store.ensure_route_toml_migrated().unwrap();
        assert_eq!(resolved, route_path);
        assert_eq!(std::fs::read_to_string(&route_path).unwrap(), legacy);
        // Second call is a no-op: existing route.toml wins even if scopes changes.
        std::fs::write(&scopes_path, "[store]\n").unwrap();
        let again = store.ensure_route_toml_migrated().unwrap();
        assert_eq!(again, route_path);
        assert_eq!(std::fs::read_to_string(&route_path).unwrap(), legacy);

        std::fs::remove_dir_all(&dir).ok();
    }
}
