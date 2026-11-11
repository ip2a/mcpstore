use crate::kv::models::ToolArgumentOverride;
use crate::store::prelude::*;

impl MCPStore {
    /// Writebacks must never touch the operator's real ~/.mcpstore/scopes.toml
    /// from a `cargo test` run: unit tests exercise these functions against
    /// whatever config_path the test built, and one stray write corrupts boot.
    /// Compiled under cfg(test) the library no-ops writeback entirely;
    /// production binaries (cfg(test) unset) always write.
    fn writeback_allowed() -> bool {
        cfg!(not(test))
    }

    /// Write back tool override to scopes.toml after setting it in openkeyv
    pub async fn writeback_tool_override(
        &self,
        service_name: &str,
        tool_name: &str,
        scope: &ScopeRef,
        arguments: &[ToolArgumentOverride],
        common: &crate::overrides::ComponentOverrideCommon,
    ) -> Result<()> {
        if !Self::writeback_allowed() {
            return Ok(());
        }
        let path = match self.scopes_config_path() {
            Ok(p) => p,
            Err(error) => {
                tracing::warn!(%error, "Cannot resolve scopes.toml path, skipping writeback");
                return Ok(());
            }
        };

        let mut doc = match load_or_empty_doc(&path) {
            Ok(doc) => doc,
            Err(error) => {
                tracing::warn!(%error, "Cannot read scopes.toml, skipping writeback");
                return Ok(());
            }
        };

        // Navigate safely: toml_edit's Index READ panics on a missing key, so
        // every level goes through contains_key + get_mut instead of chained
        // indexing (a missing service used to panic the whole API worker).
        let services = scope_services_table_mut(&mut doc, scope)
            .ok_or_else(|| Error::new(FailureCode::Internal, "cannot represent scope in scopes.toml".to_string()))?;
        if !services.contains_key(service_name) {
            services[service_name] = toml_edit::table();
        }
        let service_table = services
            .get_mut(service_name)
            .and_then(|item| item.as_table_mut())
            .ok_or_else(|| Error::new(FailureCode::Internal, "service entry is not a table".to_string()))?;
        if !service_table.contains_key("tools") {
            service_table["tools"] = toml_edit::table();
        }
        let tools_table = service_table
            .get_mut("tools")
            .and_then(|item| item.as_table_mut())
            .ok_or_else(|| Error::new(FailureCode::Internal, "tools entry is not a table".to_string()))?;
        if !tools_table.contains_key("overrides") {
            tools_table["overrides"] = toml_edit::table();
        }
        let overrides_table = tools_table
            .get_mut("overrides")
            .and_then(|item| item.as_table_mut())
            .ok_or_else(|| Error::new(FailureCode::Internal, "overrides entry is not a table".to_string()))?;

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

    /// Remove a tool override's section from scopes.toml after deleting it
    /// from openkeyv, so the next load cannot resurrect the rule from its
    /// writeback. Mirrors writeback_service_override_removal: drop the tool's
    /// overrides entry, then prune now-empty overrides/tools/service parents.
    pub async fn writeback_tool_override_removal(
        &self,
        service_name: &str,
        tool_name: &str,
        scope: &ScopeRef,
    ) -> Result<()> {
        if !Self::writeback_allowed() {
            return Ok(());
        }
        let path = match self.scopes_config_path() {
            Ok(p) => p,
            Err(error) => {
                tracing::warn!(%error, "Cannot resolve scopes.toml path, skipping writeback");
                return Ok(());
            }
        };
        if !path.exists() {
            return Ok(());
        }

        let mut doc = parse_scopes_doc(&path)?;

        if let Some(services) = scope_services_table_mut(&mut doc, scope) {
            if let Some(service_table) = services
                .get_mut(service_name)
                .and_then(|item| item.as_table_mut())
            {
                if let Some(tools) = service_table
                    .get_mut("tools")
                    .and_then(|item| item.as_table_mut())
                {
                    if let Some(overrides) = tools
                        .get_mut("overrides")
                        .and_then(|item| item.as_table_mut())
                    {
                        overrides.remove(tool_name);
                    }
                    if tools
                        .get("overrides")
                        .and_then(|item| item.as_table())
                        .is_some_and(|table| table.is_empty())
                    {
                        tools.remove("overrides");
                    }
                }
                if service_table
                    .get("tools")
                    .and_then(|item| item.as_table())
                    .is_some_and(|table| table.is_empty())
                {
                    service_table.remove("tools");
                }
            }
            if services
                .get(service_name)
                .and_then(|item| item.as_table())
                .is_some_and(|table| table.is_empty())
            {
                services.remove(service_name);
            }
            write_scopes_doc_atomic(&path, &doc)?;
        }
        Ok(())
    }

    /// Write back a service-level override (rename/description/hide) to
    /// scopes.toml after setting it in openkeyv. Same patch semantics as the
    /// tool writeback: Some sets the key, None leaves any existing key
    /// untouched, hand-written foreign keys survive.
    pub async fn writeback_service_override(
        &self,
        service_name: &str,
        scope: &ScopeRef,
        common: &crate::overrides::ComponentOverrideCommon,
    ) -> Result<()> {
        if !Self::writeback_allowed() {
            return Ok(());
        }
        let path = match self.scopes_config_path() {
            Ok(p) => p,
            Err(error) => {
                tracing::warn!(%error, "Cannot resolve scopes.toml path, skipping writeback");
                return Ok(());
            }
        };
        let mut doc = match load_or_empty_doc(&path) {
            Ok(doc) => doc,
            Err(error) => {
                tracing::warn!(%error, "Cannot read scopes.toml, skipping writeback");
                return Ok(());
            }
        };

        if let Some(service_table) = service_scope_table_mut(&mut doc, service_name, scope) {
            apply_service_override_fields(service_table, common);
            write_scopes_doc_atomic(&path, &doc)?;
        }

        tracing::debug!("Wrote back service override for {service_name} to scopes.toml");
        Ok(())
    }

    /// Remove the writeback-owned service keys from scopes.toml so a deleted
    /// override cannot be resurrected by the next load. The service entry
    /// itself stays if it still carries tools/prompts/resources sections.
    pub async fn writeback_service_override_removal(
        &self,
        service_name: &str,
        scope: &ScopeRef,
    ) -> Result<()> {
        if !Self::writeback_allowed() {
            return Ok(());
        }
        let path = match self.scopes_config_path() {
            Ok(p) => p,
            Err(error) => {
                tracing::warn!(%error, "Cannot resolve scopes.toml path, skipping writeback");
                return Ok(());
            }
        };
        if !path.exists() {
            return Ok(());
        }

        let mut doc = parse_scopes_doc(&path)?;

        if let Some(service_table) = service_scope_table_mut(&mut doc, service_name, scope) {
            for key in [
                "display_name",
                "description",
                "enabled",
                "tags",
                "meta",
                "annotations",
                "icons",
            ] {
                service_table.remove(key);
            }
            if service_table.is_empty() {
                if let Some(services) = scope_services_table_mut(&mut doc, scope) {
                    services.remove(service_name);
                }
            }
            write_scopes_doc_atomic(&path, &doc)?;
        }

        tracing::debug!("Removed service override keys for {service_name} from scopes.toml");
        Ok(())
    }
}

/// Parse scopes.toml into an editable document. A missing file becomes an
/// empty document: the first writeback creates the file (the loader treats
/// absent/partial files as empty, so booting with only writeback entries is
/// well-defined).
fn load_or_empty_doc(path: &std::path::Path) -> Result<toml_edit::DocumentMut> {
    if !path.exists() {
        return "".parse().map_err(|e| {
            Error::new(FailureCode::Internal, format!("empty scopes.toml parse failed: {e}"))
        });
    }
    parse_scopes_doc(path)
}

/// Parse scopes.toml into an editable document.
fn parse_scopes_doc(path: &std::path::Path) -> Result<toml_edit::DocumentMut> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        Error::new(FailureCode::Internal, format!("Failed to read scopes.toml: {}", e))
    })?;
    content
        .parse()
        .map_err(|e| Error::new(FailureCode::Internal, format!("Failed to parse scopes.toml: {}", e)))
}

/// Atomic temp-file + rename write, preserving the existing tool-writeback
/// contract.
fn write_scopes_doc_atomic(path: &std::path::Path, doc: &toml_edit::DocumentMut) -> Result<()> {
    let temp_path = path.with_extension("toml.tmp");
    std::fs::write(&temp_path, doc.to_string()).map_err(|e| {
        Error::new(FailureCode::Internal, format!("Failed to write temp file: {}", e))
    })?;
    std::fs::rename(&temp_path, path).map_err(|e| {
        Error::new(FailureCode::Internal, format!("Failed to rename temp file: {}", e))
    })
}

/// Mutable `[<scope>.services.<name>]` table, creating the path when missing;
/// `None` when the scope root itself cannot be represented.
fn service_scope_table_mut<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    service_name: &str,
    scope: &ScopeRef,
) -> Option<&'a mut toml_edit::Table> {
    let services = scope_services_table_mut(doc, scope)?;
    if !services.contains_key(service_name) {
        services[service_name] = toml_edit::table();
    }
    services
        .get_mut(service_name)
        .and_then(|item| item.as_table_mut())
}

/// Mutable `[<scope>.services]` table, creating it when missing.
fn scope_services_table_mut<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    scope: &ScopeRef,
) -> Option<&'a mut toml_edit::Table> {
    let scope_table = match scope {
        ScopeRef::Store => {
            if !doc.contains_key("store") {
                doc["store"] = toml_edit::table();
            }
            &mut doc["store"]
        }
        ScopeRef::Agent { agent_id } => {
            if !doc.contains_key("agents") {
                doc["agents"] = toml_edit::table();
            }
            if !doc["agents"].as_table().map(|t| t.contains_key(agent_id)).unwrap_or(false) {
                doc["agents"][agent_id] = toml_edit::table();
            }
            &mut doc["agents"][agent_id]
        }
    };
    if !scope_table.is_table() {
        *scope_table = toml_edit::Item::Table(toml_edit::Table::default());
    }
    let scope_table = scope_table.as_table_mut()?;
    if !scope_table.contains_key("services") {
        scope_table["services"] = toml_edit::table();
    }
    scope_table["services"].as_table_mut()
}

/// Apply the writeback-owned fields of a service override onto its
/// scopes.toml table. Owned fields follow patch semantics — Some/non-empty
/// sets the key, None/empty leaves any existing key as-is.
fn apply_service_override_fields(
    table: &mut toml_edit::Table,
    common: &crate::overrides::ComponentOverrideCommon,
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
            if let Some(item) = toml_item_from_json(json) {
                table.insert(key, item);
            }
        }
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

    // Arguments (only defaults for now). The loader reads each argument as an
    // ArgumentOverride table ({ default = … }), so the default value must be
    // wrapped — a bare `debug = false` fails TOML deserialization on the next
    // load and takes boot down with it.
    if !arguments.is_empty() {
        let mut args_table = toml_edit::Table::default();
        for arg in arguments {
            if let Some(ref default) = arg.default_value {
                if let Some(item) = toml_item_from_json(default) {
                    let mut arg_table = toml_edit::Table::default();
                    arg_table.insert("default", item);
                    args_table.insert(&arg.original_name, toml_edit::Item::Table(arg_table));
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
    fn missing_scopes_toml_yields_empty_doc_for_first_write() {
        let dir = std::env::temp_dir().join(format!(
            "mcpstore-scopes-wb-missing-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scopes.toml");
        let _ = std::fs::remove_file(&path);
        let doc = load_or_empty_doc(&path).unwrap();
        assert_eq!(doc.to_string(), "");
        std::fs::remove_dir_all(&dir).ok();
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
    #[test]
    fn tool_removal_drops_entry_and_prunes_empty_parents() {
        // (writeback_tool_override_removal's table surgery lives in the same
        // navigation helpers; exercise the doc-level behavior directly.)
        let mut doc: toml_edit::DocumentMut = r#"
[store.services.filesystem.tools.overrides.write_file]
display_name = "safe_write"
enabled = true

[store.services.filesystem.tools.overrides.list_directory]
enabled = false
"#
        .parse()
        .unwrap();
        let services = doc["store"]["services"].as_table_mut().unwrap();
        let service = services.get_mut("filesystem").and_then(|i| i.as_table_mut()).unwrap();
        let tools = service.get_mut("tools").and_then(|i| i.as_table_mut()).unwrap();
        let overrides = tools.get_mut("overrides").and_then(|i| i.as_table_mut()).unwrap();
        overrides.remove("write_file");
        // list_directory survives its sibling's removal
        assert!(overrides.contains_key("list_directory"));
        overrides.remove("list_directory");
        tools.remove("overrides");
        service.remove("tools");
        services.remove("filesystem");
        let written = doc.to_string();
        assert!(!written.contains("filesystem"), "service pruned: {written}");
        assert!(!written.contains("write_file"), "tool pruned: {written}");

    }

    fn writeback_writes_argument_defaults_wrapped() {
        // The loader reads arguments as ArgumentOverride tables — a bare
        // boolean default must serialize as `debug = { default = false }`,
        // never `debug = false` (which fails TOML deserialization on the
        // next load and breaks boot).
        // Render through a DocumentMut: a standalone Table::to_string() drops
        // sub-tables, but production always serializes the whole document.
        let mut doc: toml_edit::DocumentMut = "".parse().unwrap();
        apply_tool_override_fields(
            doc.as_table_mut(),
            &common(None, None, Some(true), vec![]),
            &[crate::kv::models::ToolArgumentOverride {
                original_name: "debug".to_string(),
                new_name: None,
                hidden: true,
                description: None,
                default_value: Some(serde_json::json!(false)),
                validation_schema: None,
            }],
        );
        let written = doc.to_string();
        assert!(written.contains("debug"), "arg key missing: {written}");
        assert!(
            written.contains("default = false"),
            "default not wrapped in ArgumentOverride table: {written}"
        );
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

    #[test]
    fn service_writeback_creates_store_section_and_applies_fields() {
        let mut doc: toml_edit::DocumentMut = "".parse().unwrap();
        let table = service_scope_table_mut(&mut doc, "github", &ScopeRef::Store)
            .expect("store scope is navigable");
        apply_service_override_fields(table, &common(Some("GitHub 巨人"), None, Some(false), vec![]));
        let written = doc.to_string();
        assert!(written.contains("[store.services.github]"));
        assert!(written.contains("display_name = \"GitHub 巨人\""));
        assert!(written.contains("enabled = false"));
    }

    #[test]
    fn service_removal_clears_owned_keys_and_keeps_tool_sections() {
        let mut doc: toml_edit::DocumentMut = r#"
[store.services.github]
display_name = "GitHub"
enabled = false

[store.services.github.tools.overrides.search_repos]
display_name = "Search"
"#
        .parse()
        .unwrap();
        let table = service_scope_table_mut(&mut doc, "github", &ScopeRef::Store).unwrap();
        for key in ["display_name", "description", "enabled", "tags", "meta", "annotations", "icons"]
        {
            table.remove(key);
        }
        let written = doc.to_string();
        assert!(!written.contains("GitHub 巨人"));
        assert!(!written.contains("enabled = false"));
        // tool sections and their overrides survive
        assert!(written.contains("[store.services.github.tools.overrides.search_repos]"));
    }

    #[test]
    fn service_removal_drops_empty_service_entry() {
        let mut doc: toml_edit::DocumentMut = r#"
[store.services.github]
display_name = "GitHub"
"#
        .parse()
        .unwrap();
        let table = service_scope_table_mut(&mut doc, "github", &ScopeRef::Store).unwrap();
        for key in ["display_name", "description", "enabled", "tags", "meta", "annotations", "icons"]
        {
            table.remove(key);
        }
        if table.is_empty() {
            if let Some(services) = scope_services_table_mut(&mut doc, &ScopeRef::Store) {
                services.remove("github");
            }
        }
        let written = doc.to_string();
        assert!(!written.contains("github"), "empty entry should be dropped: {written}");
    }
}
