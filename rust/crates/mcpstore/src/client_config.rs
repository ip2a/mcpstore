//! Read-only inspection of programming-assistant MCP configuration files.
//!
//! This module deliberately stops before mutation and keeps unknown fields available
//! to callers that need to inspect or import existing service definitions.

use crate::{config::ServerConfig, Error, FailureCode, Result};
use serde_json::{Map, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Codex,
    ClaudeCode,
    OpenCode,
    Cursor,
    ClaudeDesktop,
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

pub fn import_selected_services(
    inspection: &ClientConfigInspection,
    selected_names: &[String],
) -> Result<Vec<(String, ServerConfig)>> {
    if selected_names.is_empty() {
        return Err(Error::new(
            FailureCode::Internal,
            "至少选择一个要导入的服务",
        ));
    }
    let mut seen = HashSet::new();
    selected_names
        .iter()
        .map(|name| {
            if !seen.insert(name.as_str()) {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("重复选择服务: {name}"),
                ));
            }
            let service = inspection
                .services
                .iter()
                .find(|service| service.name == *name)
                .ok_or_else(|| {
                    Error::new(
                        FailureCode::Internal,
                        format!("助手配置中不存在服务: {name}"),
                    )
                })?;
            Ok((
                name.clone(),
                imported_server_config(inspection.client, name, &service.config)?,
            ))
        })
        .collect()
}

fn imported_server_config(client: ClientKind, name: &str, value: &Value) -> Result<ServerConfig> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new(FailureCode::Internal, format!("服务 {name} 配置必须是对象")))?;
    let unsupported = object
        .keys()
        .filter(|field| !supported_fields(client).contains(&field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("服务 {name} 包含不可导入字段: {}", unsupported.join(", ")),
        ));
    }

    let (command, args, env) = if client == ClientKind::OpenCode {
        let command = match object.get("command") {
            Some(Value::Array(parts)) => {
                let mut parts = parts
                    .iter()
                    .map(|part| {
                        part.as_str().map(str::to_owned).ok_or_else(|| {
                            Error::new(
                                FailureCode::Internal,
                                format!("服务 {name} 的 command 必须是字符串数组"),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                if parts.is_empty() {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("服务 {name} 的 command 不能为空"),
                    ));
                }
                let executable = parts.remove(0);
                (Some(executable), parts)
            }
            None => (None, Vec::new()),
            _ => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("服务 {name} 的 command 必须是字符串数组"),
                ))
            }
        };
        (
            command.0,
            command.1,
            string_map(object.get("environment"), name, "environment")?,
        )
    } else {
        (
            optional_string(object.get("command"), name, "command")?,
            string_array(object.get("args"), name, "args")?,
            string_map(object.get("env"), name, "env")?,
        )
    };
    let url = optional_string(object.get("url"), name, "url")?;
    if command.is_some() == url.is_some() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("服务 {name} 必须且只能设置 command 或 url"),
        ));
    }
    if client == ClientKind::OpenCode {
        match object.get("type").and_then(Value::as_str) {
            Some("local") if command.is_none() => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("服务 {name} 的 local 类型缺少 command"),
                ))
            }
            Some("remote") if url.is_none() => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("服务 {name} 的 remote 类型缺少 url"),
                ))
            }
            Some("local" | "remote") | None => {}
            Some(kind) => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("服务 {name} 使用不支持的 OpenCode 类型: {kind}"),
                ))
            }
        }
        if object.get("enabled") == Some(&Value::Bool(false)) {
            return Err(Error::new(
                FailureCode::Internal,
                format!("服务 {name} 已在 OpenCode 中禁用"),
            ));
        }
        if object.contains_key("timeout") {
            return Err(Error::new(
                FailureCode::Internal,
                format!("服务 {name} 的 timeout 没有安全的 MCPStore 映射"),
            ));
        }
    }
    Ok(ServerConfig {
        url,
        command,
        args,
        env,
        headers: string_map(object.get("headers"), name, "headers")?,
        transport: Some(if object.contains_key("url") {
            "streamable-http".into()
        } else {
            "stdio".into()
        }),
        ..ServerConfig::default()
    })
}

fn optional_string(value: Option<&Value>, name: &str, field: &str) -> Result<Option<String>> {
    value
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    FailureCode::Internal,
                    format!("服务 {name} 的 {field} 必须是字符串"),
                )
            })
        })
        .transpose()
}

fn string_array(value: Option<&Value>, name: &str, field: &str) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("服务 {name} 的 {field} 必须是字符串数组"),
            )
        })?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    FailureCode::Internal,
                    format!("服务 {name} 的 {field} 必须是字符串数组"),
                )
            })
        })
        .collect()
}

fn string_map(value: Option<&Value>, name: &str, field: &str) -> Result<HashMap<String, String>> {
    let Some(value) = value else {
        return Ok(HashMap::new());
    };
    value
        .as_object()
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("服务 {name} 的 {field} 必须是字符串对象"),
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
                        format!("服务 {name} 的 {field}.{key} 必须是字符串"),
                    )
                })
        })
        .collect()
}

fn supported_fields(client: ClientKind) -> &'static [&'static str] {
    match client {
        ClientKind::Codex
        | ClientKind::ClaudeCode
        | ClientKind::Cursor
        | ClientKind::ClaudeDesktop => &["command", "args", "env", "url", "headers"],
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
    let bytes = fs::read(&path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("无法读取 {}: {error}", path.display()),
        )
    })?;
    let (format, document) = match client {
        ClientKind::Codex => (
            ConfigFormat::Toml,
            toml::from_str::<toml::Value>(&String::from_utf8_lossy(&bytes))
                .map_err(|error| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Codex 配置格式错误: {error}"),
                    )
                })
                .and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))
                })?,
        ),
        ClientKind::ClaudeCode | ClientKind::Cursor | ClientKind::ClaudeDesktop => (
            ConfigFormat::Json,
            serde_json::from_slice(&bytes).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("Claude Code 配置格式错误: {error}"),
                )
            })?,
        ),
        ClientKind::OpenCode => (
            ConfigFormat::Json,
            serde_json::from_slice(&bytes).map_err(|error| {
                Error::new(
                    FailureCode::Internal,
                    format!("OpenCode 配置格式错误: {error}"),
                )
            })?,
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
        ClientKind::ClaudeCode | ClientKind::Cursor | ClientKind::ClaudeDesktop => "mcpServers",
        ClientKind::OpenCode => "mcp",
    };
    match document.get(key) {
        None => Ok(&EMPTY_SERVICES),
        Some(value) => value
            .as_object()
            .ok_or_else(|| Error::new(FailureCode::Internal, format!("配置字段 {key} 必须是对象"))),
    }
}

static EMPTY_SERVICES: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);

fn unsupported_fields(client: ClientKind, document: &Value) -> Vec<String> {
    let Some(servers) = document
        .get(match client {
            ClientKind::Codex => "mcp_servers",
            ClientKind::ClaudeCode | ClientKind::Cursor | ClientKind::ClaudeDesktop => "mcpServers",
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
    format!("blake3:{}", blake3::hash(bytes).to_hex())
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
    fn inspects_cursor_and_claude_desktop_json() {
        for client in [ClientKind::Cursor, ClientKind::ClaudeDesktop] {
            let path = sample(match client {
                ClientKind::Cursor => "cursor",
                _ => "desktop",
            });
            let result = inspect_client_config(client, &path).unwrap();
            assert!(result.services.iter().any(|service| service.name == "demo"));
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn imports_only_explicitly_selected_services_into_server_configs() {
        let path = sample("import");
        fs::write(
            &path,
            r#"{"mcpServers":{"stdio":{"command":"node","args":["server.js"],"env":{"TOKEN":"secret"}},"remote":{"url":"http://127.0.0.1/mcp","headers":{"Authorization":"Bearer secret"}}}}"#,
        )
        .unwrap();
        let inspection = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
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
        let path =
            std::env::temp_dir().join(format!("mcpstore-import-opencode-{}", std::process::id()));
        fs::write(
            &path,
            r#"{"mcp":{"local":{"type":"local","command":["node","server.js"],"environment":{"TOKEN":"secret"}},"lossy":{"type":"remote","url":"http://127.0.0.1/mcp","timeout":1000}}}"#,
        )
        .unwrap();
        let inspection = inspect_client_config(ClientKind::OpenCode, &path).unwrap();
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
        let path = sample("bad");
        fs::write(&path, "{\"other\":1}").unwrap();
        let result = inspect_client_config(ClientKind::ClaudeCode, &path).unwrap();
        assert!(result.services.is_empty());
        let _ = fs::remove_file(path);
    }
}
