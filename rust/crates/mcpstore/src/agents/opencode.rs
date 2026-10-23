//! OpenCode: MCP servers in `~/.config/opencode/opencode.json` under `mcp`.
//! Its entries use a command *array* and an `environment` map, and carry
//! `type`/`enabled`/`timeout` fields the store can only import losslessly
//! for the subset validated below.

use serde_json::Value;
use std::path::PathBuf;

use crate::agents::{as_object, build_service, reject_unsupported, ClientSetup};
use crate::client_config::{optional_string, string_map, ClientKind, ConfigFormat};
use crate::{Error, FailureCode, Result, ServerConfig};

pub struct OpenCode;

impl ClientSetup for OpenCode {
    fn kind(&self) -> ClientKind {
        ClientKind::OpenCode
    }
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn label(&self) -> &'static str {
        "OpenCode"
    }
    fn format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }
    fn servers_key(&self) -> &'static str {
        "mcp"
    }
    fn supported_fields(&self) -> &'static [&'static str] {
        &[
            "type",
            "command",
            "url",
            "headers",
            "enabled",
            "environment",
            "timeout",
        ]
    }
    fn entry_value(&self, url: &str) -> Result<Value> {
        Ok(serde_json::json!({
            "type": "remote",
            "url": url,
            "enabled": true
        }))
    }
    fn import_service(&self, name: &str, value: &Value) -> Result<ServerConfig> {
        let object = as_object(name, value)?;
        reject_unsupported(self.supported_fields(), name, object)?;
        let (command, args) = match object.get("command") {
            Some(Value::Array(parts)) => {
                let mut parts = parts
                    .iter()
                    .map(|part| {
                        part.as_str().map(str::to_owned).ok_or_else(|| {
                            Error::new(
                                FailureCode::Internal,
                                format!("Service {name} command must be an array of strings"),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                if parts.is_empty() {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("Service {name} command cannot be empty"),
                    ));
                }
                let executable = parts.remove(0);
                (Some(executable), parts)
            }
            None => (None, Vec::new()),
            _ => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Service {name} command must be an array of strings"),
                ))
            }
        };
        let env = string_map(object.get("environment"), name, "environment")?;
        let url = optional_string(object.get("url"), name, "url")?;
        if command.is_some() == url.is_some() {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service {name} must set exactly one of command or url"),
            ));
        }
        match object.get("type").and_then(Value::as_str) {
            Some("local") if command.is_none() => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Local service {name} is missing command"),
                ))
            }
            Some("remote") if url.is_none() => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Remote service {name} is missing url"),
                ))
            }
            Some("local" | "remote") | None => {}
            Some(kind) => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Service {name} uses unsupported OpenCode type: {kind}"),
                ))
            }
        }
        if object.get("enabled") == Some(&Value::Bool(false)) {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service {name} is disabled in OpenCode"),
            ));
        }
        if object.contains_key("timeout") {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service {name} timeout has no safe MCPStore mapping"),
            ));
        }
        build_service(name, object, command, args, env)
    }
    fn default_config_paths(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("~/.config/opencode/opencode.json"),
            PathBuf::from("~/.opencode.json"),
        ]
    }
    fn config_dirs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/.config/opencode")]
    }
    fn cli_commands(&self) -> &'static [&'static str] {
        &["opencode"]
    }
}
