//! Read-only inspection of programming-assistant MCP configuration files.
//!
//! This module deliberately stops before mutation and keeps unknown fields available
//! to callers that need to inspect or import existing service definitions. The
//! per-client knowledge (where the MCP map lives, which entry fields import) lives
//! in `agents`; this module drives it through the registry.

use crate::{config::ServerConfig, Error, FailureCode, Result};
use serde_json::{Map, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConfigFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone)]
pub struct ClientConfigInspection {
    pub client_id: String,
    pub path: PathBuf,
    pub format: ConfigFormat,
    pub content_hash: String,
    pub document: Value,
    pub services: Vec<ClientMcpService>,
    pub unsupported_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientMcpService {
    pub name: String,
    pub config: Value,
}

pub fn import_selected_services(
    inspection: &ClientConfigInspection,
    selected_names: &[String],
) -> Result<Vec<(String, ServerConfig)>> {
    if selected_names.is_empty() {
        return Err(Error::new(
            FailureCode::Internal,
            "at least one service must be selected to import",
        ));
    }
    let mut seen = HashSet::new();
    selected_names
        .iter()
        .map(|name| {
            if !seen.insert(name.as_str()) {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Service selected more than once: {name}"),
                ));
            }
            let service = inspection
                .services
                .iter()
                .find(|service| service.name == *name)
                .ok_or_else(|| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Service missing from assistant configuration: {name}"),
                    )
                })?;
            Ok((
                name.clone(),
                crate::agents::by_id(&inspection.client_id)
                    .ok_or_else(|| {
                        Error::new(
                            FailureCode::Internal,
                            format!("Unknown client plugin: {}", inspection.client_id),
                        )
                    })?
                    .import_service(name, &service.config)?,
            ))
        })
        .collect()
}

pub(crate) fn string_map(
    value: Option<&Value>,
    name: &str,
    field: &str,
) -> Result<HashMap<String, String>> {
    let Some(value) = value else {
        return Ok(HashMap::new());
    };
    value
        .as_object()
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("Service {name} {field} must be a string map"),
            )
        })?
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_owned()))
                .ok_or_else(|| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Service {name} {field}.{key} must be a string"),
                    )
                })
        })
        .collect()
}

pub fn inspect_client_config(
    client_id: &str,
    path: impl AsRef<Path>,
) -> Result<ClientConfigInspection> {
    let setup = crate::agents::by_id(client_id).ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("Unknown client plugin: {client_id}"),
        )
    })?;
    let path = path.as_ref().to_path_buf();
    let bytes = fs::read(&path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", path.display()),
        )
    })?;
    let (format, document) = match setup.format() {
        ConfigFormat::Toml => (
            ConfigFormat::Toml,
            toml::from_str::<toml::Value>(&String::from_utf8_lossy(&bytes))
                .map_err(|error| {
                    Error::new(
                        FailureCode::Internal,
                        format!("{} configuration format error: {error}", setup.label()),
                    )
                })
                .and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))
                })?,
        ),
        ConfigFormat::Json => (
            ConfigFormat::Json,
            serde_json::from_slice(&bytes).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("{} configuration format error: {error}", setup.label()),
                )
            })?,
        ),
    };
    let key = setup.servers_key();
    let services = service_map(key, &document)?
        .iter()
        .map(|(name, config)| ClientMcpService {
            name: name.clone(),
            config: config.clone(),
        })
        .collect();
    let unsupported_fields = unsupported_fields(key, setup.supported_fields(), &document);
    Ok(ClientConfigInspection {
        client_id: client_id.to_string(),
        path,
        format,
        content_hash: content_hash(&bytes),
        document,
        services,
        unsupported_fields,
    })
}

fn service_map<'a>(key: &str, document: &'a Value) -> Result<&'a Map<String, Value>> {
    match document.get(key) {
        None => Ok(&EMPTY_SERVICES),
        Some(value) => value.as_object().ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("Configuration field {key} must be an object"),
            )
        }),
    }
}

static EMPTY_SERVICES: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);

fn unsupported_fields(key: &str, supported: &[&str], document: &Value) -> Vec<String> {
    let Some(servers) = document.get(key).and_then(Value::as_object) else {
        return vec![];
    };
    let mut result = Vec::new();
    for (name, config) in servers {
        let Some(object) = config.as_object() else {
            result.push(format!("{name}: entry must be an object"));
            continue;
        };
        for key in object
            .keys()
            .filter(|key| !supported.contains(&key.as_str()))
        {
            result.push(format!("{name}.{key}"));
        }
    }
    result
}

fn content_hash(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn setup() {
        crate::agents::registry::install_for_tests();
    }
    fn sample(suffix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-client-config-{suffix}-{}",
            std::process::id()
        ));
        let mut file = fs::File::create(&path).unwrap();
        write!(
            file,
            "{{\"mcpServers\":{{\"demo\":{{\"command\":\"node\",\"custom\":true}}}},\"other\":1}}"
        )
        .unwrap();
        path
    }
    #[test]
    fn inspects_claude_without_dropping_unknown_fields() {
        setup();
        let path = sample("claude");
        let result = inspect_client_config("claude", &path).unwrap();
        assert_eq!(result.services.len(), 1);
        assert_eq!(result.document["other"], 1);
        assert_eq!(result.unsupported_fields, vec!["demo.custom"]);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn inspects_codex_toml() {
        setup();
        let path = std::env::temp_dir().join(format!("mcpstore-codex-{}", std::process::id()));
        fs::write(
            &path,
            "[mcp_servers.demo]\ncommand = 'node'\nargs = ['server.js']\n",
        )
        .unwrap();
        let result = inspect_client_config("codex", &path).unwrap();
        assert_eq!(result.services[0].name, "demo");
        assert_eq!(result.format, ConfigFormat::Toml);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn inspects_opencode_local_service() {
        setup();
        let path = std::env::temp_dir().join(format!("mcpstore-opencode-{}", std::process::id()));
        fs::write(
            &path,
            "{\"mcp\":{\"demo\":{\"type\":\"local\",\"command\":[\"node\",\"server.js\"]}}}",
        )
        .unwrap();
        let result = inspect_client_config("opencode", &path).unwrap();
        assert_eq!(result.services[0].config["type"], "local");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn inspects_cursor_and_omp_json() {
        setup();
        for client in ["cursor", "omp"] {
            let path = sample(client);
            let result = inspect_client_config(client, &path).unwrap();
            assert!(result.services.iter().any(|service| service.name == "demo"));
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn unknown_client_id_is_an_error() {
        setup();
        let path = sample("unknown");
        assert!(inspect_client_config("not-a-plugin", &path).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn imports_only_explicitly_selected_services_into_server_configs() {
        setup();
        let path = sample("import");
        fs::write(
            &path,
            r#"{"mcpServers":{"stdio":{"command":"node","args":["server.js"],"env":{"TOKEN":"secret"}},"remote":{"url":"http://127.0.0.1/mcp","headers":{"Authorization":"Bearer secret"}}}}"#,
        )
        .unwrap();
        let inspection = inspect_client_config("claude", &path).unwrap();
        let imported = import_selected_services(&inspection, &["stdio".into()]).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].0, "stdio");
        assert_eq!(imported[0].1.command.as_deref(), Some("node"));
        assert_eq!(imported[0].1.args, ["server.js"]);
        assert_eq!(imported[0].1.env["TOKEN"], "secret");
        assert_eq!(imported[0].1.infer_transport(), "stdio");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn imports_opencode_command_array_and_rejects_lossy_fields() {
        setup();
        let path =
            std::env::temp_dir().join(format!("mcpstore-import-opencode-{}", std::process::id()));
        fs::write(
            &path,
            r#"{"mcp":{"local":{"type":"local","command":["node","server.js"],"environment":{"TOKEN":"secret"}},"lossy":{"type":"remote","url":"http://127.0.0.1/mcp","timeout":1000}}}"#,
        )
        .unwrap();
        let inspection = inspect_client_config("opencode", &path).unwrap();
        let imported = import_selected_services(&inspection, &["local".into()]).unwrap();
        assert_eq!(imported[0].1.command.as_deref(), Some("node"));
        assert_eq!(imported[0].1.args, ["server.js"]);
        assert_eq!(imported[0].1.env["TOKEN"], "secret");
        assert!(import_selected_services(&inspection, &["lossy".into()]).is_err());
        assert!(import_selected_services(&inspection, &["missing".into()]).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn accepts_config_without_mcp_section() {
        setup();
        let path = sample("bad");
        fs::write(&path, "{\"other\":1}").unwrap();
        let result = inspect_client_config("claude", &path).unwrap();
        assert!(result.services.is_empty());
        let _ = fs::remove_file(path);
    }
}
