//! Manifest-based client setup: a declarative `plugin.toml` drives everything
//! the store needs to detect, inspect, import, and wire one coding client.
//! No per-client Rust code: adding a client is adding a manifest.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::agents::{as_object, build_service, reject_unsupported, ClientSetup};
use crate::client_config::{string_map, ConfigFormat};
use crate::{Error, FailureCode, Result, ServerConfig};

/// Plugin manifest parsed from `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub plugin: PluginMetadata,
    pub config: ConfigSection,
    #[serde(default)]
    pub gateway_entry: Option<GatewayEntrySection>,
    pub discover: DiscoverSection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginMetadata {
    pub id: String,
    pub label: String,
    pub spec_version: u32,
    /// Plugin content version ("MAJOR.MINOR.PATCH"); compared against the
    /// market index for update hints.
    pub plugin_version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSection {
    pub format: String, // "json" | "toml"
    pub servers_key: String,
    pub supported_fields: Vec<String>,
    /// How the `command` field is written: a single string, or an array whose
    /// first element is the executable.
    #[serde(default)]
    pub command_shape: CommandShape,
    /// The key holding the environment map, when it is not `env`.
    #[serde(default)]
    pub env_key: Option<String>,
    /// When present, an entry's `type` must be one of these values, and the
    /// field it maps to (`command` or `url`) must be set. A type value outside
    /// the map is rejected.
    #[serde(default)]
    pub type_map: Option<HashMap<String, String>>,
    /// Refuse entries the user disabled in the client itself.
    #[serde(default)]
    pub reject_disabled: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandShape {
    #[default]
    String,
    Array,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayEntrySection {
    #[serde(default = "default_url_key")]
    pub url_key: String,
    #[serde(default)]
    pub extra: HashMap<String, Value>,
    #[serde(default = "default_supports_remote")]
    pub supports_remote: bool,
}

fn default_url_key() -> String {
    "url".to_string()
}

fn default_supports_remote() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverSection {
    #[serde(default)]
    pub config_paths: Vec<String>,
    #[serde(default)]
    pub config_paths_macos: Vec<String>,
    #[serde(default)]
    pub config_paths_linux: Vec<String>,
    #[serde(default)]
    pub config_paths_windows: Vec<String>,
    #[serde(default)]
    pub config_dirs: Vec<String>,
    #[serde(default)]
    pub config_dirs_macos: Vec<String>,
    #[serde(default)]
    pub config_dirs_linux: Vec<String>,
    #[serde(default)]
    pub config_dirs_windows: Vec<String>,
    #[serde(default)]
    pub cli_commands: Vec<String>,
}

/// A [`ClientSetup`] built from a manifest. The strings the trait hands out as
/// `&'static` are leaked once here, at construction; a setup lives for the
/// process lifetime, so nothing is reclaimed and nothing leaks per call.
#[derive(Debug)]
pub struct ManifestSetup {
    id: &'static str,
    label: &'static str,
    servers_key: &'static str,
    supported_fields: &'static [&'static str],
    cli_commands: &'static [&'static str],
    manifest: PluginManifest,
}

fn leak_str(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}

fn leak_strs(values: &[String]) -> &'static [&'static str] {
    Box::leak(
        values
            .iter()
            .map(|value| leak_str(value))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

/// Exactly three non-empty digit segments: "MAJOR.MINOR.PATCH".
pub(crate) fn valid_plugin_version(version: &str) -> bool {
    let mut parts = version.split('.');
    for _ in 0..3 {
        match parts.next() {
            Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()) => {}
            _ => return false,
        }
    }
    parts.next().is_none()
}

impl ManifestSetup {
    /// Parse and validate one manifest from TOML source.
    pub fn from_toml(source: &str) -> Result<Self> {
        let manifest: PluginManifest = toml::from_str(source)
            .map_err(|e| Error::new(FailureCode::Internal, format!("invalid plugin manifest: {e}")))?;

        let id = &manifest.plugin.id;
        if manifest.plugin.spec_version != 1 {
            return Err(Error::new(
                FailureCode::Internal,
                format!(
                    "plugin {id}: unsupported spec_version {}, want 1",
                    manifest.plugin.spec_version
                ),
            ));
        }
        if !valid_plugin_version(&manifest.plugin.plugin_version) {
            return Err(Error::new(
                FailureCode::Internal,
                format!(
                    "plugin {id}: plugin_version {:?} must be MAJOR.MINOR.PATCH",
                    manifest.plugin.plugin_version
                ),
            ));
        }
        match manifest.config.format.as_str() {
            "json" | "toml" => {}
            other => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("plugin {id}: unknown config format {other:?}"),
                ))
            }
        }
        if let Some(type_map) = &manifest.config.type_map {
            for (value, field) in type_map {
                if field != "command" && field != "url" {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("plugin {id}: type_map {value:?} must map to \"command\" or \"url\""),
                    ));
                }
            }
        }
        let discover = &manifest.discover;
        if discover.config_paths.is_empty()
            && discover.config_paths_macos.is_empty()
            && discover.config_paths_linux.is_empty()
            && discover.config_paths_windows.is_empty()
            && discover.config_dirs.is_empty()
            && discover.config_dirs_macos.is_empty()
            && discover.config_dirs_linux.is_empty()
            && discover.config_dirs_windows.is_empty()
            && discover.cli_commands.is_empty()
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!("plugin {id}: [discover] must list config_paths, config_dirs, or cli_commands"),
            ));
        }

        Ok(Self {
            id: leak_str(id),
            label: leak_str(&manifest.plugin.label),
            servers_key: leak_str(&manifest.config.servers_key),
            supported_fields: leak_strs(&manifest.config.supported_fields),
            cli_commands: leak_strs(&discover.cli_commands),
            manifest,
        })
    }

    fn env_key(&self) -> &str {
        self.manifest.config.env_key.as_deref().unwrap_or("env")
    }

    /// Content version from the manifest; shown in UI, compared against market.
    pub fn plugin_version(&self) -> &str {
        &self.manifest.plugin.plugin_version
    }

    /// Plugin specification version the manifest declares.
    pub fn spec_version(&self) -> u32 {
        self.manifest.plugin.spec_version
    }

    /// Paths for the current platform: the per-OS override list when set, else the base list.
    fn platform_paths(
        base: &[String],
        macos: &[String],
        linux: &[String],
        windows: &[String],
    ) -> Vec<PathBuf> {
        let paths = if cfg!(target_os = "macos") && !macos.is_empty() {
            macos
        } else if cfg!(target_os = "linux") && !linux.is_empty() {
            linux
        } else if cfg!(target_os = "windows") && !windows.is_empty() {
            windows
        } else {
            base
        };
        paths.iter().map(PathBuf::from).collect()
    }

    fn string_parts(
        &self,
        value: Option<&Value>,
        name: &str,
        field: &str,
    ) -> Result<Vec<String>> {
        let Some(value) = value else {
            return Ok(Vec::new());
        };
        value
            .as_array()
            .ok_or_else(|| {
                Error::new(
                    FailureCode::Internal,
                    format!("Service {name} {field} must be an array of strings"),
                )
            })?
            .iter()
            .map(|part| {
                part.as_str().map(str::to_owned).ok_or_else(|| {
                    Error::new(
                        FailureCode::Internal,
                        format!("Service {name} {field} must be an array of strings"),
                    )
                })
            })
            .collect()
    }

    fn extract_command(
        &self,
        name: &str,
        object: &Map<String, Value>,
    ) -> Result<(Option<String>, Vec<String>)> {
        match object.get("command") {
            None => Ok((None, Vec::new())),
            Some(value) => match self.manifest.config.command_shape {
                CommandShape::String => {
                    let command = value.as_str().ok_or_else(|| {
                        Error::new(
                            FailureCode::Internal,
                            format!("Service {name} command must be a string"),
                        )
                    })?;
                    let args = self.string_parts(object.get("args"), name, "args")?;
                    Ok((Some(command.to_owned()), args))
                }
                CommandShape::Array => {
                    let mut parts = self.string_parts(Some(value), name, "command")?;
                    if parts.is_empty() {
                        return Err(Error::new(
                            FailureCode::Internal,
                            format!("Service {name} command cannot be empty"),
                        ));
                    }
                    let executable = parts.remove(0);
                    Ok((Some(executable), parts))
                }
            },
        }
    }
}

impl ClientSetup for ManifestSetup {
    fn id(&self) -> &'static str {
        self.id
    }

    fn label(&self) -> &'static str {
        self.label
    }

    fn format(&self) -> ConfigFormat {
        match self.manifest.config.format.as_str() {
            "json" => ConfigFormat::Json,
            _ => ConfigFormat::Toml,
        }
    }

    fn servers_key(&self) -> &'static str {
        self.servers_key
    }

    fn supported_fields(&self) -> &'static [&'static str] {
        self.supported_fields
    }

    fn entry_value(&self, url: &str) -> Result<Value> {
        let Some(gateway) = &self.manifest.gateway_entry else {
            return Err(Error::new(
                FailureCode::Internal,
                format!("{} does not support remote MCP servers", self.label),
            ));
        };
        if !gateway.supports_remote {
            return Err(Error::new(
                FailureCode::Internal,
                format!("{} does not support remote MCP servers", self.label),
            ));
        }
        let mut entry = Map::new();
        entry.insert(gateway.url_key.clone(), json!(url));
        for (key, value) in &gateway.extra {
            entry.insert(key.clone(), value.clone());
        }
        Ok(Value::Object(entry))
    }

    fn import_service(&self, name: &str, value: &Value) -> Result<ServerConfig> {
        let object = as_object(name, value)?;
        reject_unsupported(self.supported_fields, name, object)?;

        if self.manifest.config.reject_disabled && object.get("enabled") == Some(&Value::Bool(false))
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service {name} is disabled in {}", self.label),
            ));
        }

        if let Some(type_map) = &self.manifest.config.type_map {
            if let Some(kind) = object.get("type").and_then(Value::as_str) {
                let Some(required) = type_map.get(kind) else {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("Service {name} uses unsupported type: {kind}"),
                    ));
                };
                if !object.contains_key(required.as_str()) {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!("{kind} service {name} is missing {required}"),
                    ));
                }
            }
        }

        let (command, args) = self.extract_command(name, object)?;
        let env = string_map(object.get(self.env_key()), name, self.env_key())?;
        build_service(name, object, command, args, env)
    }

    fn default_config_paths(&self) -> Vec<PathBuf> {
        let d = &self.manifest.discover;
        Self::platform_paths(
            &d.config_paths,
            &d.config_paths_macos,
            &d.config_paths_linux,
            &d.config_paths_windows,
        )
    }

    fn config_dirs(&self) -> Vec<PathBuf> {
        let d = &self.manifest.discover;
        Self::platform_paths(
            &d.config_dirs,
            &d.config_dirs_macos,
            &d.config_dirs_linux,
            &d.config_dirs_windows,
        )
    }

    fn cli_commands(&self) -> &'static [&'static str] {
        self.cli_commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_builtin_manifest() {
        for (id, source) in [
            ("claude", include_str!("../../../../../plugins/claude/plugin.toml")),
            ("codex", include_str!("../../../../../plugins/codex/plugin.toml")),
            ("opencode", include_str!("../../../../../plugins/opencode/plugin.toml")),
            ("cursor", include_str!("../../../../../plugins/cursor/plugin.toml")),
            ("pi", include_str!("../../../../../plugins/pi/plugin.toml")),
            ("omp", include_str!("../../../../../plugins/omp/plugin.toml")),
        ] {
            let setup = ManifestSetup::from_toml(source)
                .unwrap_or_else(|e| panic!("builtin {id} failed to parse: {e}"));
            assert_eq!(setup.id(), id);
            assert!(!setup.label().is_empty());
            assert!(
                !setup.default_config_paths().is_empty()
                    || !setup.config_dirs().is_empty()
                    || !setup.cli_commands().is_empty()
            );
        }
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = ManifestSetup::from_toml(
            r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"
typo_field = true

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]

[discover]
config_paths = ["~/x.json"]
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    #[test]
    fn plugin_version_must_be_semver_triplet() {
        let source = |version: &str| {
            format!(
                r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "{version}"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]

[discover]
config_paths = ["~/x.json"]
"#
            )
        };
        let err = ManifestSetup::from_toml(&source("1.0")).unwrap_err();
        assert!(err.to_string().contains("plugin_version"), "{err}");
        let err = ManifestSetup::from_toml(&source("1.0.0.0")).unwrap_err();
        assert!(err.to_string().contains("plugin_version"), "{err}");
        let setup = ManifestSetup::from_toml(&source("1.2.10")).unwrap();
        assert_eq!(setup.manifest.plugin.plugin_version, "1.2.10");
    }

    #[test]
    fn wrong_spec_version_names_the_wanted_version() {
        let err = ManifestSetup::from_toml(
            r#"
[plugin]
id = "x"
label = "X"
spec_version = 2
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]

[discover]
config_paths = ["~/x.json"]
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("spec_version 2"), "{err}");
    }

    #[test]
    fn discover_must_not_be_empty() {
        let err = ManifestSetup::from_toml(
            r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]

[discover]
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("[discover]"), "{err}");
    }

    #[test]
    fn type_map_rejects_unknown_values_and_missing_fields() {
        let source = r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["type", "command", "url"]

[config.type_map]
stdio = "command"
http = "url"

[discover]
config_paths = ["~/x.json"]
"#;
        let setup = ManifestSetup::from_toml(source).unwrap();

        let good = setup
            .import_service("a", &serde_json::json!({"type": "stdio", "command": "node"}))
            .unwrap();
        assert_eq!(good.command.as_deref(), Some("node"));

        let unknown = setup
            .import_service("b", &serde_json::json!({"type": "sse", "url": "http://x/mcp"}))
            .unwrap_err();
        assert!(unknown.to_string().contains("unsupported type"), "{unknown}");

        let missing = setup
            .import_service("c", &serde_json::json!({"type": "http"}))
            .unwrap_err();
        assert!(missing.to_string().contains("missing url"), "{missing}");

        let no_type = setup
            .import_service("d", &serde_json::json!({"command": "node"}))
            .unwrap();
        assert_eq!(no_type.command.as_deref(), Some("node"));
    }

    #[test]
    fn env_key_drives_the_environment_lookup() {
        let source = r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command", "environment"]
env_key = "environment"

[discover]
config_paths = ["~/x.json"]
"#;
        let setup = ManifestSetup::from_toml(source).unwrap();
        let service = setup
            .import_service("a", &serde_json::json!({"command": "node", "environment": {"T": "1"}}))
            .unwrap();
        assert_eq!(service.env["T"], "1");
    }

    #[test]
    fn array_command_splits_executable() {
        let source = r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command"]
command_shape = "array"

[discover]
config_paths = ["~/x.json"]
"#;
        let setup = ManifestSetup::from_toml(source).unwrap();
        let service = setup
            .import_service("a", &serde_json::json!({"command": ["node", "server.js"]}))
            .unwrap();
        assert_eq!(service.command.as_deref(), Some("node"));
        assert_eq!(service.args, ["server.js"]);

        let empty = setup
            .import_service("b", &serde_json::json!({"command": []}))
            .unwrap_err();
        assert!(empty.to_string().contains("cannot be empty"), "{empty}");
    }

    #[test]
    fn gateway_entry_absent_or_remote_refused() {
        let no_gateway = ManifestSetup::from_toml(
            r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command", "url"]

[discover]
config_paths = ["~/x.json"]
"#,
        )
        .unwrap();
        assert!(no_gateway.entry_value("http://g/mcp").is_err());

        let no_remote = ManifestSetup::from_toml(
            r#"
[plugin]
id = "y"
label = "Y"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command", "url"]

[gateway_entry]
supports_remote = false

[discover]
config_paths = ["~/y.json"]
"#,
        )
        .unwrap();
        assert!(no_remote.entry_value("http://g/mcp").is_err());

        let setup = ManifestSetup::from_toml(
            r#"
[plugin]
id = "z"
label = "Z"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command", "url"]

[gateway_entry]
extra = { type = "remote", enabled = true }

[discover]
config_paths = ["~/z.json"]
"#,
        )
        .unwrap();
        assert_eq!(
            setup.entry_value("http://g/mcp").unwrap(),
            serde_json::json!({"url": "http://g/mcp", "type": "remote", "enabled": true})
        );
    }

    #[test]
    fn reject_disabled_refuses_enabled_false() {
        let source = r#"
[plugin]
id = "x"
label = "X"
spec_version = 1
plugin_version = "1.0.0"

[config]
format = "json"
servers_key = "mcpServers"
supported_fields = ["command", "enabled"]
reject_disabled = true

[discover]
config_paths = ["~/x.json"]
"#;
        let setup = ManifestSetup::from_toml(source).unwrap();
        let err = setup
            .import_service("a", &serde_json::json!({"command": "node", "enabled": false}))
            .unwrap_err();
        assert!(err.to_string().contains("disabled"), "{err}");
    }
}
