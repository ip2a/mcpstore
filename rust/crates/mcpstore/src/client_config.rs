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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Codex,
    ClaudeCode,
    OpenCode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientEntryKind {
    Original,
    AggregateStdio,
    AggregateHttp,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
        .get(service_key(inspection.client))
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConfigChangeReceipt {
    pub client: ClientKind,
    pub path: PathBuf,
    pub backup_path: PathBuf,
    pub before_hash: String,
    pub after_hash: String,
}

pub fn apply_config_change(
    inspection: &ClientConfigInspection,
    plans: &[ClientEntryPlan],
) -> Result<Option<ConfigChangeReceipt>> {
    let current_bytes = fs::read(&inspection.path).map_err(|error| {
        StoreError::Other(format!(
            "无法重新读取 {}: {error}",
            inspection.path.display()
        ))
    })?;
    if content_hash(&current_bytes) != inspection.content_hash {
        return Err(StoreError::Other(
            "配置文件已被外部修改，请重新检查后再写入".into(),
        ));
    }
    if let Some(plan) = plans.iter().find(|plan| {
        matches!(
            plan.status,
            ClientEntryStatus::Conflict | ClientEntryStatus::Unsupported
        )
    }) {
        return Err(StoreError::Other(format!(
            "配置计划 {} 为 {:?}，拒绝写入",
            plan.name, plan.status
        )));
    }
    let additions: Vec<_> = plans
        .iter()
        .filter(|plan| plan.status == ClientEntryStatus::New)
        .collect();
    if additions.is_empty() {
        return Ok(None);
    }

    let mut document = inspection.document.clone();
    let key = service_key(inspection.client);
    let root = document
        .as_object_mut()
        .ok_or_else(|| StoreError::Other("配置根节点必须是对象".into()))?;
    let services = root
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| StoreError::Other(format!("配置字段 {key} 必须是对象")))?;
    for plan in additions {
        services.insert(plan.name.clone(), plan.proposed.clone());
    }

    let output = serialize_document(inspection.format.clone(), &document)?;
    let backup_path = inspection
        .path
        .with_extension(format!("mcpstore.{}.bak", inspection.content_hash));
    fs::write(&backup_path, &current_bytes).map_err(|error| {
        StoreError::Other(format!("无法备份 {}: {error}", backup_path.display()))
    })?;
    atomic_write(&inspection.path, &output)?;
    let after = inspect_client_config(inspection.client, &inspection.path)?;
    for plan in plans
        .iter()
        .filter(|plan| plan.status != ClientEntryStatus::Same)
    {
        if after
            .services
            .iter()
            .find(|service| service.name == plan.name)
            .map(|service| &service.config)
            != Some(&plan.proposed)
        {
            return Err(StoreError::Other(format!("写入后验证失败: {}", plan.name)));
        }
    }
    Ok(Some(ConfigChangeReceipt {
        client: inspection.client,
        path: inspection.path.clone(),
        backup_path,
        before_hash: inspection.content_hash.clone(),
        after_hash: after.content_hash,
    }))
}

pub fn undo_last_change(receipt: &ConfigChangeReceipt) -> Result<()> {
    let current = fs::read(&receipt.path).map_err(|error| {
        StoreError::Other(format!("无法读取 {}: {error}", receipt.path.display()))
    })?;
    if content_hash(&current) != receipt.after_hash {
        return Err(StoreError::Other("配置文件已被外部修改，拒绝撤销".into()));
    }
    let backup = fs::read(&receipt.backup_path).map_err(|error| {
        StoreError::Other(format!(
            "无法读取备份 {}: {error}",
            receipt.backup_path.display()
        ))
    })?;
    if content_hash(&backup) != receipt.before_hash {
        return Err(StoreError::Other("备份内容校验失败，拒绝撤销".into()));
    }
    atomic_write(&receipt.path, &backup)?;
    let restored = fs::read(&receipt.path).map_err(|error| StoreError::Other(error.to_string()))?;
    if content_hash(&restored) != receipt.before_hash {
        return Err(StoreError::Other("撤销后验证失败".into()));
    }
    Ok(())
}

fn service_key(client: ClientKind) -> &'static str {
    match client {
        ClientKind::Codex => "mcp_servers",
        ClientKind::ClaudeCode => "mcpServers",
        ClientKind::OpenCode => "mcp",
    }
}

fn serialize_document(format: ConfigFormat, document: &Value) -> Result<Vec<u8>> {
    match format {
        ConfigFormat::Json => serde_json::to_vec_pretty(document)
            .map_err(|error| StoreError::Other(error.to_string())),
        ConfigFormat::Toml => toml::Value::try_from(document.clone())
            .map_err(|error| StoreError::Other(format!("Codex 配置序列化失败: {error}")))
            .and_then(|value| {
                toml::to_string_pretty(&value)
                    .map(String::into_bytes)
                    .map_err(|error| StoreError::Other(error.to_string()))
            }),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let temporary =
        path.with_file_name(format!(".{file_name}.mcpstore.tmp.{}", std::process::id()));
    fs::write(&temporary, bytes).map_err(|error| {
        StoreError::Other(format!("无法写入临时文件 {}: {error}", temporary.display()))
    })?;
    if let Ok(metadata) = fs::metadata(path) {
        let _ = fs::set_permissions(&temporary, metadata.permissions());
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(StoreError::Other(format!(
            "无法原子替换 {}: {error}",
            path.display()
        )));
    }
    Ok(())
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
    fn applies_atomically_and_refuses_undo_after_external_change() {
        let path = sample("apply");
        let inspection = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        let plans = plan_add_entries(
            &inspection,
            [ClientEntrySpec {
                name: "new".into(),
                kind: ClientEntryKind::AggregateHttp,
                config: serde_json::json!({"url":"http://127.0.0.1:9000/mcp"}),
            }],
        );
        let receipt = apply_config_change(&inspection, &plans).unwrap().unwrap();
        let written = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        assert_eq!(
            written
                .services
                .iter()
                .find(|service| service.name == "new")
                .unwrap()
                .config["url"],
            "http://127.0.0.1:9000/mcp"
        );
        assert_eq!(written.document["other"], 1);
        undo_last_change(&receipt).unwrap();
        assert!(inspect_client_config(ClientKind::ClaudeCode, &path)
            .unwrap()
            .services
            .iter()
            .all(|service| service.name != "new"));

        let inspection = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        let plans = plan_add_entries(
            &inspection,
            [ClientEntrySpec {
                name: "new".into(),
                kind: ClientEntryKind::Original,
                config: serde_json::json!({"command":"python"}),
            }],
        );
        let receipt = apply_config_change(&inspection, &plans).unwrap().unwrap();
        fs::write(&path, r#"{"mcpServers":{},"other":2}"#).unwrap();
        assert!(undo_last_change(&receipt).is_err());
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(receipt.backup_path);
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
