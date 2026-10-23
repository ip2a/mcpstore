//! Write-back of the gateway connection into assistant MCP configuration files.
//!
//! The read-only half lives in `client_config.rs`; this module is the mutation half.
//! Rules (mirroring what a careful editor owes the user's files):
//!
//! - Back up the target file before the first write (`<path>.mcpstore-bak`, overwritten
//!   on every change — a single rolling backup).
//! - Write via a temp file in the same directory + rename.
//! - Record everything we wrote in a state file (`gateway-wired.json` beside the app
//!   config). Removal only ever deletes what that record says we wrote; an entry the
//!   user edited since (URL mismatch) is theirs and removal is refused.
//!
//! ponytail: JSON clients are serialized pretty (serde_json with preserve_order keeps
//! key order, but indentation is normalized and JSONC comments are dropped); Codex TOML
//! is append-only (`[mcp_servers.<name>]` table at end of file, comments preserved).
//! A byte-level span-splice editor (magpie's approach) can replace this when loss shows.

use crate::client_config::ClientKind;
use crate::{Error, FailureCode, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents::ClientSetup;

/// The server name we write into each client's MCP map.
pub const GATEWAY_ENTRY_NAME: &str = "mcpstore";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiredEntry {
    pub client: ClientKind,
    pub path: PathBuf,
    pub name: String,
    pub url: String,
    pub applied_at_unix_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WiredState {
    #[serde(default)]
    pub entries: Vec<WiredEntry>,
}

pub fn load_wired_state(path: &Path) -> Result<WiredState> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to parse {}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(WiredState::default()),
        Err(error) => Err(Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", path.display()),
        )),
    }
}

fn save_wired_state(path: &Path, state: &WiredState) -> Result<()> {
    let body = serde_json::to_vec_pretty(state).map_err(|error| {
        Error::new(FailureCode::Internal, error.to_string())
    })?;
    atomic_write(path, &body)
}

fn atomic_write(path: &Path, body: &[u8]) -> Result<()> {
    let temp = path.with_extension("mcpstore-tmp");
    fs::write(&temp, body).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to write {}: {error}", temp.display()),
        )
    })?;
    fs::rename(&temp, path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to rename {}: {error}", path.display()),
        )
    })
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

fn backup(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let backup_path = path.with_extension("mcpstore-bak");
    fs::copy(path, &backup_path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to back up {}: {error}", backup_path.display()),
        )
    })?;
    Ok(())
}

/// Insert (or refresh) the gateway entry in the client's MCP configuration and record it.
pub fn wire_gateway(
    client: ClientKind,
    path: &Path,
    url: &str,
    state_path: &Path,
) -> Result<WiredEntry> {
    let setup = crate::agents::get(client);
    let mut state = load_wired_state(state_path)?;
    if state.entries.iter().any(|entry| entry.client == client) {
        return Err(Error::new(
            FailureCode::Internal,
            format!("Client {client:?} is already wired; unwire it first"),
        ));
    }
    let entry = WiredEntry {
        client,
        path: path.to_path_buf(),
        name: GATEWAY_ENTRY_NAME.to_string(),
        url: url.to_string(),
        applied_at_unix_ms: now_unix_ms(),
    };
    setup.wire_entry(path, url)?;
    state.entries.push(entry.clone());
    save_wired_state(state_path, &state)?;
    Ok(entry)
}

pub(crate) fn wire_json_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    path: &Path,
    url: &str,
) -> Result<()> {
    let key = setup.servers_key();
    let value = setup.entry_value(url)?;
    let mut document: Value = if path.exists() {
        let bytes = fs::read(path).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to read {}: {error}", path.display()),
            )
        })?;
        serde_json::from_slice(&bytes).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to parse {}: {error}", path.display()),
            )
        })?
    } else {
        json!({})
    };
    let root = document.as_object_mut().ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("{} must contain a JSON object", path.display()),
        )
    })?;
    let servers = root
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()));
    let map = servers.as_object_mut().ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("{} must contain a JSON object", key),
        )
    })?;
    if map.contains_key(GATEWAY_ENTRY_NAME) {
        return Err(Error::new(
            FailureCode::Internal,
            format!("{} already defines {GATEWAY_ENTRY_NAME}", path.display()),
        ));
    }
    backup(path)?;
    map.insert(GATEWAY_ENTRY_NAME.to_string(), value);
    let body = serde_json::to_vec_pretty(&document)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
    atomic_write(path, &body)
}

pub(crate) fn wire_codex<T: ClientSetup + ?Sized>(setup: &T, path: &Path, url: &str) -> Result<()> {
    let key = setup.servers_key();
    if path.exists() {
        let text = fs::read_to_string(path).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to read {}: {error}", path.display()),
            )
        })?;
        let parsed: toml::Value = toml::from_str(&text).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("Failed to parse {}: {error}", path.display()),
            )
        })?;
        let table = parsed.get(key).and_then(toml::Value::as_table);
        if table
            .map(|table| table.contains_key(GATEWAY_ENTRY_NAME))
            .unwrap_or(false)
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!("{} already defines {GATEWAY_ENTRY_NAME}", path.display()),
            ));
        }
        // An inline `mcp_servers = {...}` table can't be extended by a dotted header.
        if table.is_none() && parsed.get(key).is_some() {
            return Err(Error::new(
                FailureCode::Internal,
                "mcp_servers is an inline table; cannot append a section",
            ));
        }
        backup(path)?;
        let mut text = text;
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&format!(
            "\n[mcp_servers.{GATEWAY_ENTRY_NAME}]\nurl = \"{url}\"\n"
        ));
        atomic_write(path, text.as_bytes())
    } else {
        atomic_write(
            path,
            format!("[mcp_servers.{GATEWAY_ENTRY_NAME}]\nurl = \"{url}\"\n").as_bytes(),
        )
    }
}

/// Remove the gateway entry we recorded for a client; refuse when the user edited it.
pub fn unwire_gateway(client: ClientKind, state_path: &Path) -> Result<WiredEntry> {
    let setup = crate::agents::get(client);
    let mut state = load_wired_state(state_path)?;
    let index = state
        .entries
        .iter()
        .position(|entry| entry.client == client)
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("Client {client:?} is not wired"),
            )
        })?;
    let entry = state.entries[index].clone();
    setup.unwire_entry(&entry)?;
    state.entries.remove(index);
    save_wired_state(state_path, &state)?;
    Ok(entry)
}

pub(crate) fn unwire_json_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    entry: &WiredEntry,
) -> Result<()> {
    let bytes = fs::read(&entry.path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", entry.path.display()),
        )
    })?;
    let mut document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to parse {}: {error}", entry.path.display()),
        )
    })?;
    let map = document
        .as_object_mut()
        .and_then(|root| root.get_mut(setup.servers_key()))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("{} has no MCP server map", entry.path.display()),
            )
        })?;
    let Some(current) = map.get(GATEWAY_ENTRY_NAME) else {
        // Our entry is already gone; just drop the record.
        return Ok(());
    };
    let current = current.clone();
    // An entry the user edited since is theirs; removal is refused.
    if current != setup.entry_value(&entry.url)? {
        return Err(Error::new(
            FailureCode::Internal,
            format!(
                "{} entry {GATEWAY_ENTRY_NAME} was edited by hand; refusing to remove it",
                entry.path.display()
            ),
        ));
    }
    backup(&entry.path)?;
    map.remove(GATEWAY_ENTRY_NAME);
    let body = serde_json::to_vec_pretty(&document)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
    atomic_write(&entry.path, &body)
}

pub(crate) fn unwire_codex<T: ClientSetup + ?Sized>(setup: &T, entry: &WiredEntry) -> Result<()> {
    let text = fs::read_to_string(&entry.path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", entry.path.display()),
        )
    })?;
    let header = format!("[mcp_servers.{}]", GATEWAY_ENTRY_NAME);
    let Some(start) = text.find(&header) else {
        return Ok(()); // already gone; just drop the record
    };
    let mut end = text.len();
    if let Some((offset, _)) = text[start + header.len()..].match_indices("\n[").next() {
        end = start + header.len() + offset + 1;
    }
    let body = text[start..end].to_string();
    let parsed: toml::Value = toml::from_str(&body).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to parse {} entry: {error}", entry.path.display()),
        )
    })?;
    let current_url = parsed
        .get(setup.servers_key())
        .and_then(|servers| servers.get(GATEWAY_ENTRY_NAME))
        .and_then(|table| table.get("url"))
        .and_then(toml::Value::as_str);
    if current_url != Some(entry.url.as_str()) {
        return Err(Error::new(
            FailureCode::Internal,
            format!(
                "{} entry {GATEWAY_ENTRY_NAME} was edited by hand; refusing to remove it",
                entry.path.display()
            ),
        ));
    }
    backup(&entry.path)?;
    // Trim the leading blank line we added on wire so repeated wire/unwire stays stable.
    let mut before = &text[..start];
    if before.ends_with("\n\n") {
        before = &before[..before.len() - 1];
    }
    atomic_write(&entry.path, format!("{before}{}", &text[end..]).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(suffix: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mcpstore-wire-{suffix}-{}",
            std::process::id()
        ));
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn wire_and_unwire_claude_code() {
        let path = temp_file(
            "claude",
            "{\"mcpServers\":{\"demo\":{\"command\":\"node\"}},\"other\":1}",
        );
        let state = std::env::temp_dir().join(format!("mcpstore-wired-{}", std::process::id()));
        let entry = wire_gateway(
            ClientKind::ClaudeCode,
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            document["mcpServers"]["mcpstore"]["url"],
            "http://127.0.0.1:1830/mcp"
        );
        // Existing keys keep their order with preserve_order.
        assert_eq!(text.find("\"mcpServers\"").unwrap() < text.find("\"other\"").unwrap(), true);
        assert!(path.with_extension("mcpstore-bak").exists());
        let removed = unwire_gateway(ClientKind::ClaudeCode, &state).unwrap();
        assert_eq!(removed.url, entry.url);
        let document: Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(document["mcpServers"].get("mcpstore").is_none());
        assert_eq!(document["mcpServers"]["demo"]["command"], "node");
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn wire_codex_appends_a_table_and_unwire_removes_it() {
        let path = temp_file(
            "codex",
            "# my codex config\nmodel = \"o3\"\n\n[mcp_servers.demo]\ncommand = \"node\"\n",
        );
        let state = std::env::temp_dir().join(format!("mcpstore-wired-codex-{}", std::process::id()));
        wire_gateway(
            ClientKind::Codex,
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# my codex config"));
        assert!(text.contains("[mcp_servers.mcpstore]\nurl = \"http://127.0.0.1:1830/mcp\""));
        unwire_gateway(ClientKind::Codex, &state).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("mcp_servers.mcpstore"));
        assert!(text.contains("[mcp_servers.demo]"));
        assert!(text.starts_with("# my codex config"));
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn unwire_refuses_a_hand_edited_entry() {
        let path = temp_file("edited", "{}");
        let state = std::env::temp_dir().join(format!("mcpstore-wired-ed-{}", std::process::id()));
        wire_gateway(
            ClientKind::Cursor,
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap().replace("1830", "9999");
        fs::write(&path, text).unwrap();
        assert!(unwire_gateway(ClientKind::Cursor, &state).is_err());
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }
}
