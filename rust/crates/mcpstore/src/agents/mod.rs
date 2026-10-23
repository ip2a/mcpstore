//! Per coding-client setup: where its MCP configuration lives, what shape its
//! gateway entry takes, and how to wire and unwire it. Every client is a
//! declarative manifest (`plugins/<id>/plugin.toml`) driven by
//! [`manifest_setup::ManifestSetup`]; `client_config` and `client_wire`
//! dispatch through [`all`] and [`by_id`].

pub mod manifest_setup;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{Error, FailureCode, Result, ServerConfig};

/// Everything the store needs to set up one coding client.
pub trait ClientSetup: Sync {
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
    fn wire_entry(&self, path: &Path, url: &str) -> Result<()> {
        match self.format() {
            ConfigFormat::Json => crate::client_wire::wire_json_entry(self, path, url),
            ConfigFormat::Toml => crate::client_wire::wire_toml_entry(self, path, url),
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
    fn refresh_entry_url(
        &self,
        entry: &crate::client_wire::WiredEntry,
        url: &str,
    ) -> Result<()> {
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

/// Every client the store knows, in UI order. Parsed once, leaked for the
/// process lifetime; a built-in manifest that fails to load is a build bug
/// and panics rather than shipping a silently missing client.
pub fn all() -> &'static [&'static dyn ClientSetup] {
    static SETUPS: std::sync::LazyLock<&'static [&'static dyn ClientSetup]> =
        std::sync::LazyLock::new(|| {
            const BUILTINS: &[(&str, &str)] = &[
                ("claude", include_str!("../../../../../plugins/claude/plugin.toml")),
                ("codex", include_str!("../../../../../plugins/codex/plugin.toml")),
                ("opencode", include_str!("../../../../../plugins/opencode/plugin.toml")),
                ("cursor", include_str!("../../../../../plugins/cursor/plugin.toml")),
                ("pi", include_str!("../../../../../plugins/pi/plugin.toml")),
                ("omp", include_str!("../../../../../plugins/omp/plugin.toml")),
            ];
            let setups: Vec<&'static dyn ClientSetup> = BUILTINS
                .iter()
                .map(|(id, source)| match manifest_setup::ManifestSetup::from_toml(source) {
                    Ok(setup) => {
                        let leaked: &'static manifest_setup::ManifestSetup =
                            Box::leak(Box::new(setup));
                        leaked as &'static dyn ClientSetup
                    }
                    Err(error) => panic!("built-in client plugin {id} failed to load: {error}"),
                })
                .collect();
            Box::leak(setups.into_boxed_slice())
        });
    *SETUPS
}

/// The setup whose [`ClientSetup::id`] is `id`.
pub fn by_id(id: &str) -> Option<&'static dyn ClientSetup> {
    all().iter().copied().find(|setup| setup.id() == id)
}

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
    for path in expand_tilde(setup.default_config_paths()) {
        if path.is_file() {
            return Some(Detection {
                path,
                by: DetectedBy::ConfigFile,
            });
        }
    }
    for dir in expand_tilde(setup.config_dirs()) {
        if dir.is_dir() {
            return Some(Detection {
                path: dir,
                by: DetectedBy::ConfigDir,
            });
        }
    }
    find_command(setup.cli_commands()).map(|path| Detection {
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
                Error::new(FailureCode::Internal, format!("Service {name} url must be a string"))
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
    fn registry_lists_every_builtin_in_ui_order() {
        let ids: Vec<_> = all().iter().map(|setup| setup.id()).collect();
        assert_eq!(ids, ["claude", "codex", "opencode", "cursor", "pi", "omp"]);
        assert_eq!(by_id("claude").expect("claude").label(), "Claude Code");
        assert_eq!(by_id("pi").expect("pi").label(), "Pi");
        assert_eq!(by_id("omp").expect("omp").label(), "Oh My Pi");
        assert!(by_id("unknown").is_none());
    }

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
