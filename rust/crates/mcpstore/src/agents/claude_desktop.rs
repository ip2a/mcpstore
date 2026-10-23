//! Claude Desktop: MCP servers in `claude_desktop_config.json` under
//! `mcpServers`. It only launches stdio servers, so wiring the gateway
//! (a remote URL) is refused outright.

use serde_json::Value;
use std::path::PathBuf;

use crate::agents::{as_object, build_service, reject_unsupported, standard_fields, ClientSetup};
use crate::client_config::{ClientKind, ConfigFormat};
use crate::{Error, FailureCode, Result, ServerConfig};

pub struct ClaudeDesktop;

impl ClientSetup for ClaudeDesktop {
    fn kind(&self) -> ClientKind {
        ClientKind::ClaudeDesktop
    }
    fn id(&self) -> &'static str {
        "claude-desktop"
    }
    fn label(&self) -> &'static str {
        "Claude Desktop"
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
    fn entry_value(&self, _url: &str) -> Result<Value> {
        Err(Error::new(
            FailureCode::Internal,
            "Claude Desktop does not support remote MCP servers",
        ))
    }
    fn import_service(&self, name: &str, value: &Value) -> Result<ServerConfig> {
        let object = as_object(name, value)?;
        reject_unsupported(self.supported_fields(), name, object)?;
        let (command, args, env) = standard_fields(name, object)?;
        build_service(name, object, command, args, env)
    }
    #[cfg(target_os = "macos")]
    fn default_config_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(
            "~/Library/Application Support/Claude/claude_desktop_config.json",
        )]
    }
    #[cfg(target_os = "macos")]
    fn config_dirs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/Library/Application Support/Claude")]
    }
    #[cfg(not(target_os = "macos"))]
    fn default_config_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(
            "~/AppData/Roaming/Claude/claude_desktop_config.json",
        )]
    }
    #[cfg(not(target_os = "macos"))]
    fn config_dirs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("~/AppData/Roaming/Claude")]
    }
}
