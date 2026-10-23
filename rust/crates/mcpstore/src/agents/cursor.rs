//! Cursor: MCP servers in `~/.cursor/mcp.json` under `mcpServers`.

use serde_json::{json, Value};
use std::path::PathBuf;

use crate::agents::{as_object, build_service, reject_unsupported, standard_fields, ClientSetup};
use crate::client_config::{ClientKind, ConfigFormat};
use crate::{Result, ServerConfig};

pub struct Cursor;

impl ClientSetup for Cursor {
    fn kind(&self) -> ClientKind {
        ClientKind::Cursor
    }
    fn id(&self) -> &'static str {
        "cursor"
    }
    fn label(&self) -> &'static str {
        "Cursor"
    }
    fn format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }
    fn servers_key(&self) -> &'static str {
        "mcpServers"
    }
    fn supported_fields(&self) -> &'static [&'static str] {
        &["command", "args", "env", "url", "headers"]
    }
    fn entry_value(&self, url: &str) -> Result<Value> {
        Ok(json!({ "url": url }))
    }
    fn import_service(&self, name: &str, value: &Value) -> Result<ServerConfig> {
        let object = as_object(name, value)?;
        reject_unsupported(self.supported_fields(), name, object)?;
        let (command, args, env) = standard_fields(name, object)?;
        build_service(name, object, command, args, env)
    }
    fn default_config_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/.cursor/mcp.json")]
    }
    fn config_dirs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/.cursor")]
    }
    fn cli_commands(&self) -> &'static [&'static str] {
        &["cursor"]
    }
}
