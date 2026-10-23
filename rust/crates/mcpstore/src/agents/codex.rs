//! Codex: MCP servers in `~/.codex/config.toml` under `[mcp_servers.*]`.

use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::agents::{as_object, build_service, reject_unsupported, standard_fields, ClientSetup};
use crate::client_config::{ClientKind, ConfigFormat};
use crate::client_wire::WiredEntry;
use crate::{Error, FailureCode, Result, ServerConfig};

pub struct Codex;

impl ClientSetup for Codex {
    fn kind(&self) -> ClientKind {
        ClientKind::Codex
    }
    fn id(&self) -> &'static str {
        "codex"
    }
    fn label(&self) -> &'static str {
        "Codex"
    }
    fn format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }
    fn servers_key(&self) -> &'static str {
        "mcp_servers"
    }
    fn supported_fields(&self) -> &'static [&'static str] {
        &["command", "args", "env", "url", "headers"]
    }
    fn entry_value(&self, _url: &str) -> Result<Value> {
        Err(Error::new(
            FailureCode::Internal,
            "Codex uses TOML; wire via the TOML path",
        ))
    }
    fn import_service(&self, name: &str, value: &Value) -> Result<ServerConfig> {
        let object = as_object(name, value)?;
        reject_unsupported(self.supported_fields(), name, object)?;
        let (command, args, env) = standard_fields(name, object)?;
        build_service(name, object, command, args, env)
    }
    fn wire_entry(&self, path: &Path, url: &str) -> Result<()> {
        crate::client_wire::wire_codex(self, path, url)
    }
    fn unwire_entry(&self, entry: &WiredEntry) -> Result<()> {
        crate::client_wire::unwire_codex(self, entry)
    }
    fn default_config_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/.codex/config.toml")]
    }
    fn config_dirs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/.codex")]
    }
    fn cli_commands(&self) -> &'static [&'static str] {
        &["codex"]
    }
}
