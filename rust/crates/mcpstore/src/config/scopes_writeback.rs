use crate::config::{ComponentOverride, ScopesConfig, ServiceScopeConfig, ToolsScopeConfig};
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
        let path = match ScopesConfig::default_path() {
            Some(p) => p,
            None => {
                tracing::warn!("Cannot determine scopes.toml path, skipping writeback");
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

        // Build override item
        let mut override_item = toml_edit::Table::default();

        if let Some(ref name) = common.display_name {
            override_item.insert("display_name", toml_edit::value(name.clone()));
        }
        if let Some(ref desc) = common.description {
            override_item.insert("description", toml_edit::value(desc.clone()));
        }
        if let Some(enabled) = common.enabled {
            override_item.insert("enabled", toml_edit::value(enabled));
        }
        if !common.tags.is_empty() {
            let mut tags_array = toml_edit::Array::new();
            for tag in &common.tags {
                tags_array.push(tag.as_str());
            }
            override_item.insert("tags", toml_edit::value(tags_array));
        }

        // Arguments (only defaults for now)
        if !arguments.is_empty() {
            let mut args_table = toml_edit::Table::default();
            for arg in arguments {
                if let Some(ref default) = arg.default_value {
                    let mut arg_item = toml_edit::Table::default();
                    // Convert serde_json::Value to toml_edit::Value
                    if let Ok(toml_val) = toml_value_from_json(default) {
                        arg_item.insert("default", toml_edit::Item::Value(toml_val));
                    }
                    args_table.insert(&arg.original_name, toml_edit::Item::Table(arg_item));
                }
            }
            if !args_table.is_empty() {
                override_item.insert("arguments", toml_edit::Item::Table(args_table));
            }
        }

        overrides_table.insert(tool_name, toml_edit::Item::Table(override_item));

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

fn toml_value_from_json(json: &serde_json::Value) -> std::result::Result<toml_edit::Value, String> {
    match json {
        // TOML has no null; caller skips inserting the key when this is absent.
        serde_json::Value::Null => Err("null has no TOML representation".to_string()),
        serde_json::Value::Bool(b) => Ok(toml_edit::Value::Boolean(toml_edit::Formatted::new(*b))),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml_edit::Value::Integer(toml_edit::Formatted::new(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(toml_edit::Value::Float(toml_edit::Formatted::new(f)))
            } else {
                Err("Unsupported number".to_string())
            }
        }
        serde_json::Value::String(s) => Ok(toml_edit::Value::String(toml_edit::Formatted::new(s.clone()))),
        _ => Err("Complex JSON types not supported in TOML writeback".to_string()),
    }
}
