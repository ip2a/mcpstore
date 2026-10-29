//! Per coding-client setup: where its MCP configuration lives, what shape its
//! gateway entry takes, and how to wire and unwire it. Every client is a
//! declarative manifest (`plugin.toml`) driven by
//! [`manifest_setup::ManifestSetup`]; `client_config` and `client_wire`
//! dispatch through [`all`] and [`by_id`], which read the disk-backed
//! registry in [`registry`].

pub mod manifest_setup;
pub mod market;
pub mod registry;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{Error, FailureCode, Result, ServerConfig};

pub use registry::{
    all, by_id, load_trace, plugins_dir, registry_report, reload, save_trace, DetectedClient,
    DetectionTrace, PluginReport,
};

/// Everything the store needs to set up one coding client.
pub trait ClientSetup: Send + Sync {
    /// Stable id: the gateway URL segment (`/mcp/<id>`) and UI key.
    fn id(&self) -> &'static str;
    /// Human name shown in the UI.
    fn label(&self) -> &'static str;
    // ---- configuration shape ----
    fn format(&self) -> ConfigFormat;
    /// The document key this client lists MCP servers under.
    fn servers_key(&self) -> &'static str;
    /// Entry fields this client's MCP schema accepts; anything else is
    /// reported as unsupported and blocks import.
    fn supported_fields(&self) -> &'static [&'static str];
    /// The gateway entry as this client's document stores it. Clients whose
    /// wiring is not a JSON value (Codex's TOML table) return an error here
    /// and write through [`ClientSetup::wire_entry`] instead.
    fn entry_value(&self, url: &str) -> Result<Value>;
    /// Convert one of the client's MCP entries into a store service config.
    fn import_service(&self, name: &str, value: &Value) -> Result<ServerConfig>;
    // ---- gateway wiring ----
    /// Insert the gateway entry into the client's document. JSON documents
    /// write a JSON object; TOML documents append a dotted-header table.
    /// An identical prior entry is adopted (recording it without touching the
    /// file); a differing one is reported as [`EntryWrite::Diverged`] unless
    /// `force` replaces it.
    fn wire_entry(
        &self,
        path: &Path,
        url: &str,
        force: bool,
    ) -> Result<crate::client_wire::EntryWrite> {
        match self.format() {
            ConfigFormat::Json => crate::client_wire::wire_json_entry(self, path, url, force),
            ConfigFormat::Toml => crate::client_wire::wire_toml_entry(self, path, url, force),
        }
    }
    /// Remove the recorded gateway entry; refuse a hand-edited one.
    fn unwire_entry(&self, entry: &crate::client_wire::WiredEntry) -> Result<()> {
        match self.format() {
            ConfigFormat::Json => crate::client_wire::unwire_json_entry(self, entry),
            ConfigFormat::Toml => crate::client_wire::unwire_toml_entry(self, entry),
        }
    }
    /// Point the recorded gateway entry at a new URL; refuse a hand-edited one.
    fn refresh_entry_url(&self, entry: &crate::client_wire::WiredEntry, url: &str) -> Result<()> {
        match self.format() {
            ConfigFormat::Json => crate::client_wire::refresh_json_entry(self, entry, url),
            ConfigFormat::Toml => crate::client_wire::refresh_toml_entry(self, entry, url),
        }
    }
    // ---- discovery ----
    /// Config file candidates in priority order (`~` expanded).
    fn default_config_paths(&self) -> Vec<PathBuf>;
    /// Folders whose presence suggests the client, weaker than a config file.
    fn config_dirs(&self) -> Vec<PathBuf>;
    /// Executables that suggest an install, the last resort.
    fn cli_commands(&self) -> &'static [&'static str] {
        &[]
    }
}

pub use crate::client_config::ConfigFormat;

// ---- discovery ----

/// How a client was found on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectedBy {
    ConfigFile,
    ConfigDir,
    Command,
}

/// Where a client was found, and its config file when one exists.
#[derive(Debug, Clone)]
pub struct Detection {
    pub path: PathBuf,
    pub by: DetectedBy,
}

/// Find one client on this machine: its config file, else a known folder,
/// else one of its commands on the (extended) PATH.
pub fn detect(setup: &dyn ClientSetup) -> Option<Detection> {
    detect_by_discovery(
        setup.default_config_paths(),
        setup.config_dirs(),
        setup
            .cli_commands()
            .iter()
            .map(|command| command.to_string())
            .collect(),
    )
}

/// Discovery over raw rules, shared by the registry (`detect`) and market
/// detection (which has manifests only in the index, not on disk).
pub fn detect_by_discovery(
    config_paths: Vec<PathBuf>,
    config_dirs: Vec<PathBuf>,
    cli_commands: Vec<String>,
) -> Option<Detection> {
    for path in expand_tilde(config_paths) {
        if path.is_file() {
            return Some(Detection {
                path,
                by: DetectedBy::ConfigFile,
            });
        }
    }
    for dir in expand_tilde(config_dirs) {
        if dir.is_dir() {
            return Some(Detection {
                path: dir,
                by: DetectedBy::ConfigDir,
            });
        }
    }
    let commands: Vec<&str> = cli_commands.iter().map(String::as_str).collect();
    find_command(&commands).map(|path| Detection {
        path,
        by: DetectedBy::Command,
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn expand_tilde(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return paths;
    };
    paths
        .into_iter()
        .map(|path| {
            let Ok(stripped) = path.strip_prefix("~") else {
                return path;
            };
            home.join(stripped)
        })
        .collect()
}

fn find_command(commands: &[&str]) -> Option<PathBuf> {
    if commands.is_empty() {
        return None;
    }
    let mut dirs: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if let Some(home) = home_dir() {
        for extra in [".local/bin", ".bun/bin"] {
            let dir = home.join(extra);
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    for brew in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let dir = PathBuf::from(brew);
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs.iter()
        .flat_map(|dir| commands.iter().map(move |command| dir.join(command)))
        .find(|candidate| candidate.is_file())
}

/// Best-effort semver from the client's first CLI on PATH (`--version`, then `-V`).
pub fn cli_version(setup: &dyn ClientSetup) -> Option<String> {
    setup
        .cli_commands()
        .iter()
        .find_map(|command| query_command_version(command))
}

fn query_command_version(command: &str) -> Option<String> {
    for args in [["--version"], ["-V"]] {
        let output = std::process::Command::new(command)
            .args(args)
            .output()
            .ok()?;
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(version) = extract_version_token(stdout.trim()) {
            return Some(version);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(version) = extract_version_token(stderr.trim()) {
            return Some(version);
        }
    }
    None
}

fn extract_version_token(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    for token in text.split_whitespace() {
        let trimmed = token.trim_matches(|c| c == 'v' || c == '(' || c == ')' || c == ',');
        if looks_like_version(trimmed) {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn looks_like_version(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

// ---- shared import helpers (used by `ManifestSetup::import_service`) ----

pub(crate) fn as_object<'a>(name: &str, value: &'a Value) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("Service {name} configuration must be an object"),
        )
    })
}

pub(crate) fn reject_unsupported(
    supported: &[&str],
    name: &str,
    object: &Map<String, Value>,
) -> Result<()> {
    let unsupported = object
        .keys()
        .filter(|field| !supported.contains(&field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unsupported.is_empty() {
        return Ok(());
    }
    Err(Error::new(
        FailureCode::Internal,
        format!(
            "Service {name} contains fields that cannot be imported: {}",
            unsupported.join(", ")
        ),
    ))
}

/// Build the store service config after per-client field extraction: exactly
/// one of command and url, and the transport inferred from which is present.
pub(crate) fn build_service(
    name: &str,
    object: &Map<String, Value>,
    command: Option<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
) -> Result<ServerConfig> {
    let url = object
        .get("url")
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    FailureCode::Internal,
                    format!("Service {name} url must be a string"),
                )
            })
        })
        .transpose()?;
    if command.is_some() == url.is_some() {
        return Err(Error::new(
            FailureCode::Internal,
            format!("Service {name} must set exactly one of command or url"),
        ));
    }
    Ok(ServerConfig {
        url,
        command,
        args,
        env,
        headers: crate::client_config::string_map(object.get("headers"), name, "headers")?,
        transport: Some(if object.contains_key("url") {
            "streamable-http".into()
        } else {
            "stdio".into()
        }),
        ..ServerConfig::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tilde_expands_against_home() {
        let paths = expand_tilde(vec![PathBuf::from("~/.claude.json")]);
        if let Some(home) = home_dir() {
            assert_eq!(paths[0], home.join(".claude.json"));
        }
        let plain = expand_tilde(vec![PathBuf::from("/abs/path.json")]);
        assert_eq!(plain[0], PathBuf::from("/abs/path.json"));
    }

    struct Probe {
        file: PathBuf,
        dir: PathBuf,
    }

    impl ClientSetup for Probe {
        fn id(&self) -> &'static str {
            "probe"
        }
        fn label(&self) -> &'static str {
            "Probe"
        }
        fn format(&self) -> ConfigFormat {
            ConfigFormat::Json
        }
        fn servers_key(&self) -> &'static str {
            "mcpServers"
        }
        fn supported_fields(&self) -> &'static [&'static str] {
            &[]
        }
        fn entry_value(&self, _url: &str) -> Result<Value> {
            unreachable!()
        }
        fn import_service(&self, _name: &str, _value: &Value) -> Result<ServerConfig> {
            unreachable!()
        }
        fn default_config_paths(&self) -> Vec<PathBuf> {
            vec![self.file.clone()]
        }
        fn config_dirs(&self) -> Vec<PathBuf> {
            vec![self.dir.clone()]
        }
        fn cli_commands(&self) -> &'static [&'static str] {
            &["mcpstore-probe-not-a-real-command"]
        }
    }

    #[test]
    fn detect_falls_through_file_dir_command() {
        let root = std::env::temp_dir().join(format!("mcpstore-detect-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("config.json");
        let dir = root.join("data");
        let probe = Probe { file, dir };

        // Config file wins.
        fs::write(&probe.file, "{}").unwrap();
        assert_eq!(detect(&probe).unwrap().by, DetectedBy::ConfigFile);
        // Then the config dir.
        fs::remove_file(&probe.file).unwrap();
        fs::create_dir(&probe.dir).unwrap();
        assert_eq!(detect(&probe).unwrap().by, DetectedBy::ConfigDir);
        // Then nothing: command misses and the dir is gone.
        fs::remove_dir(&probe.dir).unwrap();
        assert!(detect(&probe).is_none());
        let _ = fs::remove_dir_all(root);
    }
}
