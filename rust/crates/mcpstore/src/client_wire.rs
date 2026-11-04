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

use crate::{Error, FailureCode, Result, ScopeRef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents::ClientSetup;

/// The server name we write into each client's MCP map.
pub const GATEWAY_ENTRY_NAME: &str = "mcpstore";

/// Exposure configuration: which management tool families are visible in the gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExposeConfig {
    #[serde(default)]
    pub session_state: bool,
    #[serde(default)]
    pub tool: bool,
    #[serde(default)]
    pub prompt: bool,
    #[serde(default)]
    pub resource: bool,
    #[serde(default)]
    pub openapi: bool,
    #[serde(default)]
    pub service: bool,
    #[serde(default)]
    pub cache: bool,
    #[serde(default)]
    pub search: bool,
}

impl Default for ExposeConfig {
    fn default() -> Self {
        Self {
            session_state: false,
            tool: false,
            prompt: false,
            resource: false,
            openapi: false,
            service: false,
            cache: false,
            search: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiredEntry {
    pub client_id: String,
    pub path: PathBuf,
    pub name: String,
    pub url: String,
    pub applied_at_unix_ms: u64,
    /// Per-client scope override; if None, defaults to Agent { agent_id: client_id }.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeRef>,
    /// Per-client exposure config; if None, defaults to ExposeConfig::default() (all false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expose: Option<ExposeConfig>,
}

impl WiredEntry {
    /// Returns the effective scope for this client: explicit scope field or derived from client_id.
    pub fn effective_scope(&self) -> Result<ScopeRef> {
        if let Some(scope) = &self.scope {
            return Ok(scope.clone());
        }
        let setup = crate::agents::by_id(&self.client_id).ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!(
                    "Client plugin {} is not installed; rewire the client or remove the stale entry",
                    self.client_id
                ),
            )
        })?;
        Ok(ScopeRef::Agent {
            agent_id: setup.id().to_string(),
        })
    }

    /// Returns the effective exposure config: explicit expose field or all-false default.
    pub fn effective_expose(&self) -> ExposeConfig {
        self.expose.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WiredState {
    #[serde(default)]
    pub entries: Vec<WiredEntry>,
}

/// Default ledger location: `gateway-wired.json` beside the app config.
pub fn wired_state_default_path() -> PathBuf {
    crate::config::ConfigManager::new()
        .app_config_path()
        .parent()
        .map(|dir| dir.join("gateway-wired.json"))
        .unwrap_or_else(|| PathBuf::from("gateway-wired.json"))
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
    let body = serde_json::to_vec_pretty(state)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
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

/// What a wire attempt did to the client's document.
#[derive(Debug, PartialEq)]
pub enum EntryWrite {
    /// No prior entry (or `force` overwrote one); the document was written.
    Written,
    /// A prior entry identical to ours; the document was untouched.
    Adopted,
    /// A prior entry differing from ours; the document was untouched.
    Diverged(Value),
}

/// Terminal result of [`wire_gateway`]: wired, or blocked by a diverging prior entry.
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum WireOutcome {
    Wired(WiredEntry),
    Conflict {
        path: PathBuf,
        current: Value,
        expected: Value,
    },
}

/// Wire a gateway URL into the client's MCP config, persisting the entry in wired state.
///
/// `scope` and `expose`: if provided, stored explicitly; if None, will use defaults on load
/// (scope derives from client_id, expose defaults to all false). An identical prior entry
/// is adopted into the ledger unchanged; a differing one is reported as [`WireOutcome::Conflict`]
/// unless `force` replaces it.
pub fn wire_gateway(
    client_id: &str,
    path: &Path,
    url: &str,
    state_path: &Path,
    scope: Option<ScopeRef>,
    expose: Option<ExposeConfig>,
    force: bool,
) -> Result<WireOutcome> {
    let setup = crate::agents::by_id(client_id).ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("Unknown client plugin: {client_id}"),
        )
    })?;
    let mut state = load_wired_state(state_path)?;
    if state
        .entries
        .iter()
        .any(|entry| entry.client_id == client_id)
    {
        return Err(Error::new(
            FailureCode::Internal,
            format!("Client {client_id} is already wired; unwire it first"),
        ));
    }
    let entry = WiredEntry {
        client_id: client_id.to_string(),
        path: path.to_path_buf(),
        name: GATEWAY_ENTRY_NAME.to_string(),
        url: url.to_string(),
        applied_at_unix_ms: now_unix_ms(),
        scope,
        expose,
    };
    let write = setup.wire_entry(path, url, force)?;
    match write {
        EntryWrite::Diverged(current) => {
            return Ok(WireOutcome::Conflict {
                path: path.to_path_buf(),
                current,
                expected: setup.entry_value(url)?,
            });
        }
        EntryWrite::Adopted | EntryWrite::Written => {}
    }
    state.entries.push(entry.clone());
    save_wired_state(state_path, &state)?;
    Ok(WireOutcome::Wired(entry))
}

pub(crate) fn wire_json_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    path: &Path,
    url: &str,
    force: bool,
) -> Result<EntryWrite> {
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
    let servers = root.entry(key).or_insert_with(|| Value::Object(Map::new()));
    let map = servers.as_object_mut().ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("{} must contain a JSON object", key),
        )
    })?;
    if let Some(current) = map.get(GATEWAY_ENTRY_NAME).cloned() {
        if current == value {
            return Ok(EntryWrite::Adopted);
        }
        if !force {
            return Ok(EntryWrite::Diverged(current));
        }
    }
    backup(path)?;
    map.insert(GATEWAY_ENTRY_NAME.to_string(), value);
    let body = serde_json::to_vec_pretty(&document)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
    atomic_write(path, &body).map(|()| EntryWrite::Written)
}

pub(crate) fn wire_toml_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    path: &Path,
    url: &str,
    force: bool,
) -> Result<EntryWrite> {
    let key = setup.servers_key();
    let header = format!("[{key}.{GATEWAY_ENTRY_NAME}]");
    let section = format!("\n{header}\nurl = \"{url}\"\n");
    if !path.exists() {
        let fresh = format!("{header}\nurl = \"{url}\"\n");
        return atomic_write(path, fresh.as_bytes()).map(|()| EntryWrite::Written);
    }
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
    // An inline `mcp_servers = {...}` table can't be extended by a dotted header.
    if table.is_none() && parsed.get(key).is_some() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("{key} is an inline table; cannot append a section"),
        ));
    }
    let current_url = table
        .and_then(|t| t.get(GATEWAY_ENTRY_NAME))
        .and_then(|t| t.get("url"))
        .and_then(toml::Value::as_str);
    if let Some(current) = current_url {
        if current == url {
            return Ok(EntryWrite::Adopted);
        }
        if !force {
            return Ok(EntryWrite::Diverged(json!({ "url": current })));
        }
    }
    let mut text = text;
    if current_url.is_some() {
        // Splice out the stale section so the append below can't duplicate it.
        if let Some(start) = text.find(&header) {
            let mut end = text.len();
            if let Some((offset, _)) = text[start + header.len()..].match_indices("\n[").next() {
                end = start + header.len() + offset + 1;
            }
            text.replace_range(start..end, "");
        }
    }
    backup(path)?;
    if !text.ends_with('\n') && !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&section);
    atomic_write(path, text.as_bytes()).map(|()| EntryWrite::Written)
}

/// Remove the gateway entry we recorded for a client; refuse when the user edited it.
pub fn unwire_gateway(client_id: &str, state_path: &Path) -> Result<WiredEntry> {
    let setup = crate::agents::by_id(client_id).ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("Unknown client plugin: {client_id}"),
        )
    })?;
    let mut state = load_wired_state(state_path)?;
    let index = state
        .entries
        .iter()
        .position(|entry| entry.client_id == client_id)
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                format!("Client {client_id} is not wired"),
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

pub(crate) fn unwire_toml_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    entry: &WiredEntry,
) -> Result<()> {
    let text = fs::read_to_string(&entry.path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", entry.path.display()),
        )
    })?;
    let header = format!("[{}.{}]", setup.servers_key(), GATEWAY_ENTRY_NAME);
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

/// Point every wired entry at its caller-mounted URL (`/mcp/<id>`). Legacy
/// entries (url without the caller segment) are rewritten only when the user
/// has not touched the file since wiring; hand-edited ones are skipped with a
/// warning. Entries whose plugin is no longer installed are skipped too.
/// Runs at daemon startup, before the faces build their mounts.
pub fn refresh_wired_urls(state_path: &Path, url_for: impl Fn(&str) -> String) {
    let Ok(mut state) = load_wired_state(state_path) else {
        return;
    };
    let mut changed = false;
    for entry in &mut state.entries {
        let expected = url_for(&entry.client_id);
        if entry.url == expected {
            continue;
        }
        let Some(setup) = crate::agents::by_id(&entry.client_id) else {
            tracing::warn!(
                client_id = %entry.client_id,
                path = %entry.path.display(),
                "skipping wired url refresh: client plugin not installed"
            );
            continue;
        };
        match setup.refresh_entry_url(entry, &expected) {
            Ok(()) => {
                entry.url = expected;
                changed = true;
            }
            Err(error) => tracing::warn!(
                client_id = %entry.client_id,
                path = %entry.path.display(),
                %error,
                "skipping wired url refresh"
            ),
        }
    }
    if changed {
        let _ = save_wired_state(state_path, &state);
    }
}

pub(crate) fn refresh_json_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    entry: &WiredEntry,
    url: &str,
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
    let Some(map) = document
        .as_object_mut()
        .and_then(|root| root.get_mut(setup.servers_key()))
        .and_then(Value::as_object_mut)
    else {
        return Ok(()); // the MCP map is gone; just re-point the record
    };
    let Some(current) = map.get(GATEWAY_ENTRY_NAME) else {
        return Ok(()); // our entry is gone; just re-point the record
    };
    if current != &setup.entry_value(&entry.url)? {
        return Err(Error::new(
            FailureCode::Internal,
            format!(
                "{} entry {GATEWAY_ENTRY_NAME} was edited by hand; refusing to update it",
                entry.path.display()
            ),
        ));
    }
    backup(&entry.path)?;
    map.insert(GATEWAY_ENTRY_NAME.to_string(), setup.entry_value(url)?);
    let body = serde_json::to_vec_pretty(&document)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
    atomic_write(&entry.path, &body)
}

pub(crate) fn refresh_toml_entry<T: ClientSetup + ?Sized>(
    setup: &T,
    entry: &WiredEntry,
    url: &str,
) -> Result<()> {
    let text = fs::read_to_string(&entry.path).map_err(|error| {
        Error::new(
            FailureCode::Internal,
            format!("Failed to read {}: {error}", entry.path.display()),
        )
    })?;
    let header = format!("[{}.{}]", setup.servers_key(), GATEWAY_ENTRY_NAME);
    let Some(start) = text.find(&header) else {
        return Ok(()); // our section is gone; just re-point the record
    };
    let mut end = text.len();
    if let Some((offset, _)) = text[start + header.len()..].match_indices("\n[").next() {
        end = start + header.len() + offset + 1;
    }
    let old_line = format!("url = \"{}\"", entry.url);
    let section = &text[start..end];
    if !section.contains(&old_line) {
        return Err(Error::new(
            FailureCode::Internal,
            format!(
                "{} entry {GATEWAY_ENTRY_NAME} was edited by hand; refusing to update it",
                entry.path.display()
            ),
        ));
    }
    backup(&entry.path)?;
    let refreshed = format!(
        "{}{}{}",
        &text[..start],
        section.replace(&old_line, &format!("url = \"{url}\"")),
        &text[end..]
    );
    atomic_write(&entry.path, refreshed.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        crate::agents::registry::install_for_tests();
    }

    fn wired(outcome: Result<WireOutcome>) -> WiredEntry {
        match outcome.unwrap() {
            WireOutcome::Wired(entry) => entry,
            WireOutcome::Conflict { .. } => panic!("expected wired, got conflict"),
        }
    }

    fn temp_file(suffix: &str, body: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("mcpstore-wire-{suffix}-{}", std::process::id()));
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn wire_and_unwire_claude_code() {
        setup();
        let path = temp_file(
            "claude",
            "{\"mcpServers\":{\"demo\":{\"command\":\"node\"}},\"other\":1}",
        );
        let state = std::env::temp_dir().join(format!("mcpstore-wired-{}", std::process::id()));
        let entry = wired(wire_gateway(
            "claude",
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
            None,
            None,
            false,
        ));
        assert_eq!(entry.client_id, "claude");
        let text = fs::read_to_string(&path).unwrap();
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            document["mcpServers"]["mcpstore"]["url"],
            "http://127.0.0.1:1830/mcp"
        );
        // Existing keys keep their order with preserve_order.
        assert!(text.find("\"mcpServers\"").unwrap() < text.find("\"other\"").unwrap());
        assert!(path.with_extension("mcpstore-bak").exists());
        let removed = unwire_gateway("claude", &state).unwrap();
        assert_eq!(removed.url, entry.url);
        let document: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(document["mcpServers"].get("mcpstore").is_none());
        assert_eq!(document["mcpServers"]["demo"]["command"], "node");
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn wire_adopts_identical_entry_and_reports_diverging_one() {
        setup();
        let state = std::env::temp_dir().join(format!("mcpstore-wired-ad-{}", std::process::id()));
        // Ledger lost, entry still present and identical → adopted, file untouched.
        let path = temp_file(
            "adopt",
            "{\"mcpServers\":{\"mcpstore\":{\"url\":\"http://127.0.0.1:1830/mcp/claude\",\"type\":\"http\"}}}",
        );
        let identical = fs::read_to_string(&path).unwrap();
        wired(wire_gateway(
            "claude",
            &path,
            "http://127.0.0.1:1830/mcp/claude",
            &state,
            None,
            None,
            false,
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), identical);
        unwire_gateway("claude", &state).unwrap();
        // Hand-edited entry with no ledger record → conflict, document untouched.
        let diverged = "{\"mcpServers\":{\"mcpstore\":{\"url\":\"http://127.0.0.1:9999/mcp\"}}}";
        fs::write(&path, diverged).unwrap();
        let WireOutcome::Conflict {
            current, expected, ..
        } = wire_gateway(
            "claude",
            &path,
            "http://127.0.0.1:1830/mcp/claude",
            &state,
            None,
            None,
            false,
        )
        .unwrap()
        else {
            panic!("expected conflict");
        };
        assert_eq!(current["url"], "http://127.0.0.1:9999/mcp");
        assert_eq!(expected["url"], "http://127.0.0.1:1830/mcp/claude");
        assert_eq!(fs::read_to_string(&path).unwrap(), diverged);
        // force → replaced, ledger records the client.
        wired(wire_gateway(
            "claude",
            &path,
            "http://127.0.0.1:1830/mcp/claude",
            &state,
            None,
            None,
            true,
        ));
        let document: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            document["mcpServers"]["mcpstore"]["url"],
            "http://127.0.0.1:1830/mcp/claude"
        );
        let state_json: Value = serde_json::from_str(&fs::read_to_string(&state).unwrap()).unwrap();
        assert_eq!(state_json["entries"].as_array().unwrap().len(), 1);
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn unknown_client_id_refuses_to_wire() {
        setup();
        let path = temp_file("ghost", "{}");
        let state = std::env::temp_dir().join(format!("mcpstore-wired-gh-{}", std::process::id()));
        let error = wire_gateway(
            "not-a-plugin",
            &path,
            "http://x/mcp",
            &state,
            None,
            None,
            false,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("Unknown client plugin"),
            "{error}"
        );
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn wire_codex_appends_a_table_and_unwire_removes_it() {
        setup();
        let path = temp_file(
            "codex",
            "# my codex config\nmodel = \"o3\"\n\n[mcp_servers.demo]\ncommand = \"node\"\n",
        );
        let state =
            std::env::temp_dir().join(format!("mcpstore-wired-codex-{}", std::process::id()));
        wired(wire_gateway(
            "codex",
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
            None,
            None,
            false,
        ));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# my codex config"));
        assert!(text.contains("[mcp_servers.mcpstore]\nurl = \"http://127.0.0.1:1830/mcp\""));
        unwire_gateway("codex", &state).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("mcp_servers.mcpstore"));
        assert!(text.contains("[mcp_servers.demo]"));
        assert!(text.starts_with("# my codex config"));
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn wire_toml_force_replaces_a_diverging_section() {
        setup();
        let path = temp_file(
            "codex-force",
            "model = \"o3\"\n\n[mcp_servers.mcpstore]\nurl = \"http://127.0.0.1:9999/mcp\"\n\n[mcp_servers.demo]\ncommand = \"node\"\n",
        );
        let state = std::env::temp_dir().join(format!("mcpstore-wired-cf-{}", std::process::id()));
        let WireOutcome::Conflict { .. } = wire_gateway(
            "codex",
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
            None,
            None,
            false,
        )
        .unwrap() else {
            panic!("expected conflict");
        };
        wired(wire_gateway(
            "codex",
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
            None,
            None,
            true,
        ));
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("9999"));
        assert!(text.contains("[mcp_servers.mcpstore]\nurl = \"http://127.0.0.1:1830/mcp\""));
        // One replacement section only — a duplicate header would be invalid TOML.
        let parsed: toml::Value = toml::from_str(&text).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["mcpstore"]["url"],
            "http://127.0.0.1:1830/mcp".into()
        );
        assert_eq!(parsed["mcp_servers"]["demo"]["command"], "node".into());
        unwire_gateway("codex", &state).unwrap();
        assert!(!fs::read_to_string(&path)
            .unwrap()
            .contains("mcp_servers.mcpstore"));
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn unwire_refuses_a_hand_edited_entry() {
        setup();
        let path = temp_file("edited", "{}");
        let state = std::env::temp_dir().join(format!("mcpstore-wired-ed-{}", std::process::id()));
        wired(wire_gateway(
            "cursor",
            &path,
            "http://127.0.0.1:1830/mcp",
            &state,
            None,
            None,
            false,
        ));
        let text = fs::read_to_string(&path).unwrap().replace("1830", "9999");
        fs::write(&path, text).unwrap();
        assert!(unwire_gateway("cursor", &state).is_err());
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn refresh_repoints_unmodified_entries_and_skips_hand_edited() {
        setup();
        let claude_path = temp_file(
            "refresh-claude",
            "{\"mcpServers\":{\"demo\":{\"command\":\"node\"},\"mcpstore\":{\"url\":\"http://127.0.0.1:1830/mcp\",\"type\":\"http\"}}}",
        );
        let state = std::env::temp_dir().join(format!("mcpstore-refresh-{}", std::process::id()));
        let legacy = format!(
            "{{\"entries\":[{{\"client_id\":\"claude\",\"path\":{},\"name\":\"mcpstore\",\"url\":\"http://127.0.0.1:1830/mcp\",\"applied_at_unix_ms\":0}}]}}",
            serde_json::to_string(&claude_path).unwrap()
        );
        fs::write(&state, legacy).unwrap();
        refresh_wired_urls(&state, |client_id| {
            assert_eq!(client_id, "claude");
            "http://127.0.0.1:1830/mcp/claude".to_string()
        });
        let document: Value =
            serde_json::from_str(&fs::read_to_string(&claude_path).unwrap()).unwrap();
        assert_eq!(
            document["mcpServers"]["mcpstore"]["url"],
            "http://127.0.0.1:1830/mcp/claude"
        );
        let wired: WiredState = serde_json::from_str(&fs::read_to_string(&state).unwrap()).unwrap();
        assert_eq!(wired.entries[0].url, "http://127.0.0.1:1830/mcp/claude");

        // A hand-edited entry keeps its file and its record url.
        let cursor_path = temp_file(
            "refresh-cursor",
            "{\"mcpServers\":{\"mcpstore\":{\"url\":\"http://127.0.0.1:1830/mcp\"}}}",
        );
        let hand_edited = format!(
            "{{\"entries\":[{{\"client_id\":\"cursor\",\"path\":{},\"name\":\"mcpstore\",\"url\":\"http://127.0.0.1:9999/mcp\",\"applied_at_unix_ms\":0}}]}}",
            serde_json::to_string(&cursor_path).unwrap()
        );
        fs::write(&state, hand_edited).unwrap();
        refresh_wired_urls(&state, |_| "http://127.0.0.1:1830/mcp/cursor".to_string());
        let document: Value =
            serde_json::from_str(&fs::read_to_string(&cursor_path).unwrap()).unwrap();
        assert_eq!(
            document["mcpServers"]["mcpstore"]["url"],
            "http://127.0.0.1:1830/mcp"
        );
        let wired: WiredState = serde_json::from_str(&fs::read_to_string(&state).unwrap()).unwrap();
        assert_eq!(wired.entries[0].url, "http://127.0.0.1:9999/mcp");
        let _ = fs::remove_file(claude_path);
        let _ = fs::remove_file(cursor_path);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn effective_scope_and_expose_derive_defaults_from_stored_entry() {
        setup();
        let document = |scope: &str| {
            format!(
                "{{\"client_id\":\"codex\",\"path\":\"/tmp/x\",\"name\":\"mcpstore\",\"url\":\"http://127.0.0.1:1830/mcp/codex\",\"applied_at_unix_ms\":0{scope}}}"
            )
        };
        let entry: WiredEntry = serde_json::from_str(&document("")).unwrap();
        assert_eq!(
            entry.effective_scope().unwrap(),
            ScopeRef::Agent {
                agent_id: "codex".to_string()
            }
        );
        assert_eq!(entry.effective_expose(), ExposeConfig::default());

        let entry: WiredEntry = serde_json::from_str(&document(
            ",\"scope\":{\"type\":\"store\"},\"expose\":{\"tool\":true}",
        ))
        .unwrap();
        assert_eq!(entry.effective_scope().unwrap(), ScopeRef::Store);
        assert!(entry.effective_expose().tool);
        assert!(!entry.effective_expose().search);
    }

    #[test]
    fn effective_scope_refuses_an_uninstalled_plugin() {
        let entry: WiredEntry = serde_json::from_str(
            "{\"client_id\":\"ghost\",\"path\":\"/tmp/x\",\"name\":\"mcpstore\",\"url\":\"http://x/mcp\",\"applied_at_unix_ms\":0}",
        )
        .unwrap();
        let error = entry.effective_scope().unwrap_err();
        assert!(error.to_string().contains("not installed"), "{error}");
    }
}
