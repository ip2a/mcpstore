use crate::cache::models::ToolArgumentOverride;
use crate::store::prelude::*;

impl MCPStore {
    /// Write back tool override to scopes.toml after setting it in openkeyv
    pub async fn writeback_tool_override(
        &self,
        service_name: &str,
        tool_name: &str,
        scope: &ScopeRef,
        arguments: &[ToolArgumentOverride],
        common: &crate::overrides::ComponentOverrideCommon,
    ) -> Result<()> {
        let path = match self.scopes_config_path() {
            Ok(p) => p,
            Err(error) => {
                tracing::warn!(%error, "Cannot resolve scopes.toml path, skipping writeback");
                return Ok(());
            }
        };

        if !path.exists() {
            tracing::debug!("scopes.toml does not exist, skipping writeback");
            return Ok(());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| Error::new(FailureCode::Internal, format!("Failed to read scopes.toml: {}", e)))?;

        let mut doc: toml_edit::DocumentMut = content
            .parse()
            .map_err(|e| Error::new(FailureCode::Internal, format!("Failed to parse scopes.toml: {}", e)))?;

        // Navigate to the right section based on scope
        let scope_key = match scope {
            ScopeRef::Store => "store",
            ScopeRef::Agent { agent_id } => {
                if !doc.contains_key("agents") {
                    doc["agents"] = toml_edit::table();
                }
                if !doc["agents"].as_table().map(|t| t.contains_key(agent_id)).unwrap_or(false) {
                    doc["agents"][agent_id] = toml_edit::table();
                }
                agent_id.as_str()
            }
        };

        // Ensure path exists
        let scope_table = if matches!(scope, ScopeRef::Store) {
            &mut doc["store"]
        } else {
            &mut doc["agents"][scope_key]
        };

        if !scope_table.is_table() {
            *scope_table = toml_edit::Item::Table(toml_edit::Table::default());
        }
        let scope_table = scope_table.as_table_mut().unwrap();

        if !scope_table.contains_key("services") {
            scope_table["services"] = toml_edit::table();
        }
        if !scope_table["services"][service_name].is_table() {
            scope_table["services"][service_name] = toml_edit::table();
        }
        if !scope_table["services"][service_name]["tools"].is_table() {
            scope_table["services"][service_name]["tools"] = toml_edit::table();
        }
        if !scope_table["services"][service_name]["tools"]["overrides"].is_table() {
            scope_table["services"][service_name]["tools"]["overrides"] = toml_edit::table();
        }

        let overrides_table = scope_table["services"][service_name]["tools"]["overrides"]
            .as_table_mut()
            .unwrap();

        // Mutate the existing override table in place so hand-written keys
        // (meta, icons, annotations) survive a writeback. Owned fields follow
        // patch semantics like the KV side: Some/non-empty sets the key,
        // None/empty leaves the existing key untouched.
        if !matches!(
            overrides_table.get(tool_name),
            Some(toml_edit::Item::Table(_))
        ) {
            overrides_table.insert(tool_name, toml_edit::Item::Table(toml_edit::Table::default()));
        }
        let override_item = overrides_table
            .get_mut(tool_name)
            .and_then(|item| item.as_table_mut())
            .expect("override item is a table");
        apply_tool_override_fields(override_item, common, arguments);

        // Write back atomically
        let temp_path = path.with_extension("toml.tmp");
        std::fs::write(&temp_path, doc.to_string())
            .map_err(|e| Error::new(FailureCode::Internal, format!("Failed to write temp file: {}", e)))?;

        std::fs::rename(&temp_path, &path)
            .map_err(|e| Error::new(FailureCode::Internal, format!("Failed to rename temp file: {}", e)))?;

        tracing::debug!("Wrote back tool override for {service_name}::{tool_name} to scopes.toml");
        Ok(())
    }
}

/// Apply the writeback-owned fields of a tool override onto its scopes.toml
/// table. Foreign keys already present on the table (hand-written extras) are
/// never touched; owned fields follow patch semantics — Some/non-empty sets
/// the key, None/empty leaves any existing key as-is.
fn apply_tool_override_fields(
    table: &mut toml_edit::Table,
    common: &crate::overrides::ComponentOverrideCommon,
    arguments: &[ToolArgumentOverride],
) {
    if let Some(ref name) = common.display_name {
        table.insert("display_name", toml_edit::value(name.clone()));
    }
    if let Some(ref desc) = common.description {
        table.insert("description", toml_edit::value(desc.clone()));
    }
    if let Some(enabled) = common.enabled {
        table.insert("enabled", toml_edit::value(enabled));
    }
    if !common.tags.is_empty() {
        let mut tags_array = toml_edit::Array::new();
        for tag in &common.tags {
            tags_array.push(tag.as_str());
        }
        table.insert("tags", toml_edit::value(tags_array));
    }
    for (key, value) in [
        ("meta", &common.meta),
        ("annotations", &common.annotations),
        ("icons", &common.icons),
    ] {
        if let Some(json) = value {
            // Unsupported JSON shapes (null, out-of-range integers) drop the
            // key rather than corrupting the file.
            if let Some(item) = toml_item_from_json(json) {
                table.insert(key, item);
            }
        }
    }

    // Arguments (only defaults for now)
    if !arguments.is_empty() {
        let mut args_table = toml_edit::Table::default();
        for arg in arguments {
            if let Some(ref default) = arg.default_value {
                if let Some(item) = toml_item_from_json(default) {
                    args_table.insert(&arg.original_name, item);
                }
            }
        }
        if !args_table.is_empty() {
            table.insert("arguments", toml_edit::Item::Table(args_table));
        }
    }
}

/// JSON → TOML item. `None` where TOML cannot represent the JSON (null;
/// integers outside i64): the caller skips such keys entirely.
fn toml_item_from_json(json: &serde_json::Value) -> Option<toml_edit::Item> {
    match json {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(value) => Some(toml_edit::value(*value)),
        serde_json::Value::Number(number) => {
            let value = number
                .as_i64()
                .map(toml_edit::Value::from)
                .or_else(|| number.as_f64().map(toml_edit::Value::from))?;
            Some(toml_edit::Item::Value(value))
        }
        serde_json::Value::String(value) => Some(toml_edit::value(value.clone())),
        serde_json::Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(toml_item_from_json(item)?.into_value().ok()?);
            }
            Some(toml_edit::Item::Value(toml_edit::Value::Array(array)))
        }
        serde_json::Value::Object(map) => {
            let mut nested = toml_edit::Table::default();
            for (key, value) in map {
                if let Some(item) = toml_item_from_json(value) {
                    nested.insert(key, item);
                }
            }
            Some(toml_edit::Item::Table(nested))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overrides::ComponentOverrideCommon;

    fn common(
        display_name: Option<&str>,
        description: Option<&str>,
        enabled: Option<bool>,
        tags: Vec<String>,
    ) -> ComponentOverrideCommon {
        ComponentOverrideCommon {
            display_name: display_name.map(str::to_string),
            description: description.map(str::to_string),
            enabled,
            tags,
            ..Default::default()
        }
    }

    #[test]
    fn writeback_preserves_foreign_keys_and_unset_fields() {
        let mut doc: toml_edit::DocumentMut = r#"
[store.services.github.tools.overrides.search_repos]
meta = "hand-written"
icons = { light = "a.png" }
enabled = false
"#
        .parse()
        .unwrap();
        let overrides_table = doc["store"]["services"]["github"]["tools"]["overrides"]
            .as_table_mut()
            .unwrap();
        let item = overrides_table
            .get_mut("search_repos")
            .and_then(|entry| entry.as_table_mut())
            .unwrap();
        apply_tool_override_fields(item, &common(Some("🔍 Search"), None, None, vec![]), &[]);

        let written = doc.to_string();
        assert!(written.contains("meta = \"hand-written\""));
        assert!(written.contains("icons"));
        // Patch did not touch enabled → hand-written value survives.
        assert!(written.contains("enabled = false"));
        assert!(written.contains("display_name = \"🔍 Search\""));
    }

    #[test]
    fn writeback_sets_owned_fields_on_fresh_table() {
        let mut table = toml_edit::Table::default();
        apply_tool_override_fields(
            &mut table,
            &common(None, Some("desc"), Some(true), vec!["t".to_string()]),
            &[],
        );
        assert!(table.get("description").is_some());
        assert!(table.get("enabled").is_some());
        assert!(table.get("tags").is_some());
        assert!(table.get("display_name").is_none());
    }

    #[test]
    fn writeback_writes_meta_and_icons_from_patch() {
        let mut doc: toml_edit::DocumentMut = r#"
[store.services.github.tools.overrides.search_repos]
"#
        .parse()
        .unwrap();
        let overrides_table = doc["store"]["services"]["github"]["tools"]["overrides"]
            .as_table_mut()
            .unwrap();
        let item = overrides_table
            .get_mut("search_repos")
            .and_then(|entry| entry.as_table_mut())
            .unwrap();
        let mut common = ComponentOverrideCommon::default();
        common.icons = Some(serde_json::json!({"light": "a.png", "dark": "b.png"}));
        common.meta = Some(serde_json::json!({"owner": "team-a", "tier": 2}));
        apply_tool_override_fields(item, &common, &[]);

        let doc = doc.to_string();
        assert!(doc.contains("a.png"));
        assert!(doc.contains("owner = \"team-a\""));
        assert!(doc.contains("tier = 2"));
    }
}
