//! Read-only inspection of programming-assistant MCP configuration files.
//!
//! This module deliberately stops before mutation: the parsed document remains the
//! source of truth for the later diff/apply milestones, so unknown fields survive.

use crate::{Result, StoreError};
use serde_json::{Map, Value};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Codex,
    ClaudeCode,
    OpenCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone)]
pub struct ClientConfigInspection {
    pub client: ClientKind,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientEntryKind {
    Original,
    AggregateStdio,
    AggregateHttp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientEntrySpec {
    pub name: String,
    pub kind: ClientEntryKind,
    pub config: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientEntryStatus {
    New,
    Same,
    Conflict,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientEntryPlan {
    pub name: String,
    pub kind: ClientEntryKind,
    pub status: ClientEntryStatus,
    pub current: Option<Value>,
    pub proposed: Value,
    pub unsupported_fields: Vec<String>,
}

pub fn plan_add_entries(
    inspection: &ClientConfigInspection,
    entries: impl IntoIterator<Item = ClientEntrySpec>,
) -> Vec<ClientEntryPlan> {
    let current = inspection
        .document
        .get(match inspection.client {
            ClientKind::Codex => "mcp_servers",
            ClientKind::ClaudeCode => "mcpServers",
            ClientKind::OpenCode => "mcp",
        })
        .and_then(Value::as_object);
    entries
        .into_iter()
        .map(|entry| {
            let unsupported_fields = entry
                .config
                .as_object()
                .map(|object| {
                    object
                        .keys()
                        .filter(|field| {
                            !supported_fields(inspection.client).contains(&field.as_str())
                        })
                        .map(|field| format!("{}.{}", entry.name, field))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![format!("{}: entry must be an object", entry.name)]);
            let current_value = current
                .and_then(|services| services.get(&entry.name))
                .cloned();
            let status = if !unsupported_fields.is_empty() {
                ClientEntryStatus::Unsupported
            } else if current_value.as_ref() == Some(&entry.config) {
                ClientEntryStatus::Same
            } else if current_value.is_some() {
                ClientEntryStatus::Conflict
            } else {
                ClientEntryStatus::New
            };
            ClientEntryPlan {
                name: entry.name,
                kind: entry.kind,
                status,
                current: current_value,
                proposed: entry.config,
                unsupported_fields,
            }
        })
        .collect()
}

fn supported_fields(client: ClientKind) -> &'static [&'static str] {
    match client {
        ClientKind::Codex | ClientKind::ClaudeCode => &["command", "args", "env", "url", "headers"],
        ClientKind::OpenCode => &[
            "type",
            "command",
            "url",
            "headers",
            "enabled",
            "environment",
            "timeout",
        ],
    }
}

pub fn inspect_client_config(
    client: ClientKind,
    path: impl AsRef<Path>,
) -> Result<ClientConfigInspection> {
    let path = path.as_ref().to_path_buf();
    let bytes = fs::read(&path)
        .map_err(|error| StoreError::Other(format!("无法读取 {}: {error}", path.display())))?;
    let (format, document) = match client {
        ClientKind::Codex => (
            ConfigFormat::Toml,
            toml::from_str::<toml::Value>(&String::from_utf8_lossy(&bytes))
                .map_err(|error| StoreError::Other(format!("Codex 配置格式错误: {error}")))
                .and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|error| StoreError::Other(error.to_string()))
                })?,
        ),
        ClientKind::ClaudeCode => (
            ConfigFormat::Json,
            serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::Other(format!("Claude Code 配置格式错误: {error}")))?,
        ),
        ClientKind::OpenCode => (
            ConfigFormat::Json,
            serde_json::from_slice(&bytes)
                .map_err(|error| StoreError::Other(format!("OpenCode 配置格式错误: {error}")))?,
        ),
    };
    let services = service_map(client, &document)?
        .iter()
        .map(|(name, config)| ClientMcpService {
            name: name.clone(),
            config: config.clone(),
        })
        .collect();
    let unsupported_fields = unsupported_fields(client, &document);
    Ok(ClientConfigInspection {
        client,
        path,
        format,
        content_hash: content_hash(&bytes),
        document,
        services,
        unsupported_fields,
    })
}

fn service_map(client: ClientKind, document: &Value) -> Result<&Map<String, Value>> {
    let key = match client {
        ClientKind::Codex => "mcp_servers",
        ClientKind::ClaudeCode => "mcpServers",
        ClientKind::OpenCode => "mcp",
    };
    match document.get(key) {
        None => Ok(&EMPTY_SERVICES),
        Some(value) => value
            .as_object()
            .ok_or_else(|| StoreError::Other(format!("配置字段 {key} 必须是对象"))),
    }
}

static EMPTY_SERVICES: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);

fn unsupported_fields(client: ClientKind, document: &Value) -> Vec<String> {
    let Some(servers) = document
        .get(match client {
            ClientKind::Codex => "mcp_servers",
            ClientKind::ClaudeCode => "mcpServers",
            ClientKind::OpenCode => "mcp",
        })
        .and_then(Value::as_object)
    else {
        return vec![];
    };
    let mut result = Vec::new();
    for (name, config) in servers {
        let Some(object) = config.as_object() else {
            result.push(format!("{name}: entry must be an object"));
            continue;
        };
        let supported = supported_fields(client);
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
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
        let path = sample("claude");
        let result = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        assert_eq!(result.services.len(), 1);
        assert_eq!(result.document["other"], 1);
        assert_eq!(result.unsupported_fields, vec!["demo.custom"]);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn inspects_codex_toml() {
        let path = std::env::temp_dir().join(format!("mcpstore-codex-{}", std::process::id()));
        fs::write(
            &path,
            "[mcp_servers.demo]\ncommand = 'node'\nargs = ['server.js']\n",
        )
        .unwrap();
        let result = inspect_client_config(ClientKind::Codex, &path).unwrap();
        assert_eq!(result.services[0].name, "demo");
        assert_eq!(result.format, ConfigFormat::Toml);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn inspects_opencode_local_service() {
        let path = std::env::temp_dir().join(format!("mcpstore-opencode-{}", std::process::id()));
        fs::write(
            &path,
            "{\"mcp\":{\"demo\":{\"type\":\"local\",\"command\":[\"node\",\"server.js\"]}}}",
        )
        .unwrap();
        let result = inspect_client_config(ClientKind::OpenCode, &path).unwrap();
        assert_eq!(result.services[0].config["type"], "local");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn plans_new_same_conflict_and_unsupported_without_mutating_document() {
        let path = sample("plan");
        fs::write(&path, r#"{"mcpServers":{"demo":{"command":"node","custom":true},"same":{"command":"node"}},"other":1}"#).unwrap();
        let inspection = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        let plans = plan_add_entries(
            &inspection,
            [
                ClientEntrySpec {
                    name: "same".into(),
                    kind: ClientEntryKind::Original,
                    config: serde_json::json!({"command":"node"}),
                },
                ClientEntrySpec {
                    name: "new".into(),
                    kind: ClientEntryKind::Original,
                    config: serde_json::json!({"command":"python"}),
                },
                ClientEntrySpec {
                    name: "demo".into(),
                    kind: ClientEntryKind::Original,
                    config: serde_json::json!({"command":"node","custom":true}),
                },
                ClientEntrySpec {
                    name: "demo".into(),
                    kind: ClientEntryKind::AggregateStdio,
                    config: serde_json::json!({"command":"other"}),
                },
                ClientEntrySpec {
                    name: "unsupported".into(),
                    kind: ClientEntryKind::AggregateHttp,
                    config: serde_json::json!({"command":"node","secret_ref":"env:X"}),
                },
            ],
        );
        assert_eq!(plans[0].status, ClientEntryStatus::Same);
        assert_eq!(plans[1].status, ClientEntryStatus::New);
        assert_eq!(plans[2].status, ClientEntryStatus::Unsupported);
        assert_eq!(plans[3].status, ClientEntryStatus::Conflict);
        assert_eq!(plans[4].status, ClientEntryStatus::Unsupported);
        assert_eq!(inspection.document["other"], 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn accepts_config_without_mcp_section() {
        let path = sample("bad");
        fs::write(&path, "{\"other\":1}").unwrap();
        let result = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        assert!(result.services.is_empty());
        let _ = fs::remove_file(path);
    }
}
