use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use mcpstore::config::{ConfigManager, HostEntry, HostsConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PanelSettings {
    pub language: String,
    /// The panel's own address book of daemons. Flat entries deserialize via
    /// HostsConfig, which also swallows unknown keys (its `entries` map is
    /// flattened), so unknown-field strictness for the rest of the file comes
    /// from HostEntry parse errors — still an explicit failure.
    #[serde(default)]
    pub hosts: HostsConfig,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            hosts: HostsConfig::default(),
        }
    }
}

static PERSISTENCE_LOCK: Mutex<()> = Mutex::new(());

/// Serializes read-modify-write cycles over panel-settings.json and the legacy
/// `[ui].language` migration inside the App process.
pub(super) fn persistence_lock() -> MutexGuard<'static, ()> {
    // Files are replaced by atomic rename, so a panic mid-write leaves either
    // the old or the new file; recovering from a poisoned lock is safe.
    PERSISTENCE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) fn load_or_migrate(manager: &ConfigManager) -> Result<PanelSettings, String> {
    let path = panel_settings_path(manager);
    let raw = manager
        .load_raw_app_config_value()
        .map_err(|error| error.to_string())?;
    if path.exists() {
        let content = std::fs::read(&path).map_err(|error| error.to_string())?;
        let mut settings: PanelSettings =
            serde_json::from_slice(&content).map_err(|error| error.to_string())?;
        // One-time adoption: a file written before hosts moved here has no
        // `hosts` key; pull the daemon config's `[hosts]` in once and rewrite.
        let has_hosts_key = serde_json::from_slice::<Value>(&content)
            .ok()
            .is_some_and(|value| value.get("hosts").is_some());
        if !has_hosts_key {
            settings.hosts = hosts_from_legacy_config(&raw);
            save(&path, &settings)?;
        }
        validate(&settings)?;
        return Ok(settings);
    }

    let language = raw
        .get("ui")
        .and_then(|ui| ui.get("language"))
        .and_then(Value::as_str);
    let settings = PanelSettings {
        language: match language {
            Some(language) => normalize_language(language)?,
            None => "en".to_string(),
        },
        hosts: hosts_from_legacy_config(&raw),
    };
    if language.is_some() || settings_has_hosts(&raw) {
        save(&path, &settings)?;
        if language.is_some() {
            remove_legacy_language(manager, raw.clone())?;
        }
    }
    Ok(settings)
}

fn settings_has_hosts(raw: &Value) -> bool {
    raw.get("hosts")
        .is_some_and(|hosts| hosts.as_object().is_some_and(|hosts| !hosts.is_empty()))
}

/// Adopt the daemon config's `[hosts]` section (legacy single-`url` entries
/// normalize inside HostsConfig's deserializer). Absent section → default.
fn hosts_from_legacy_config(raw: &Value) -> HostsConfig {
    raw.get("hosts")
        .cloned()
        .map(|hosts| serde_json::from_value(hosts).unwrap_or_default())
        .unwrap_or_default()
}

/// The local URL the panel should use: the daemon's real `[server].host:port`.
/// Falls back to `ServerSettings::default()` when the config is unreadable —
/// which reproduces the generated default URL, making the align step a no-op.
fn aligned_local_url(manager: &ConfigManager) -> String {
    let server = match manager.load_app_config_or_default() {
        Ok(config) => config.server,
        Err(_) => Default::default(),
    };
    format!("http://{}:{}", server.host, server.port)
}

/// True when the URL's host is this machine (loopback). The aligned default
/// and any previously aligned URL both qualify.
fn is_loopback_http_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .and_then(|authority| {
            authority
                .rsplit_once(':')
                .map(|(host, _)| host)
                .or(Some(authority))
        })
        .unwrap_or("")
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1")
}

/// Retarget an untouched default `local` entry to the daemon's actual
/// `[server].port`. Explicit user URLs are never modified: "untouched" means
/// the entry still points at this machine — any loopback host on any port
/// counts (a port left behind by a previous alignment or by a settings save
/// must not freeze the alignment; the S5 regression). A non-loopback URL is
/// user intent and stays untouched.
fn align_local_entry(manager: &ConfigManager, mut hosts: HostsConfig) -> HostsConfig {
    let generated_default = HostsConfig::generated_default_local_url();
    let is_default = match hosts.entries.get("local") {
        Some(entry) => entry
            .urls
            .first()
            .is_some_and(|url| is_loopback_http_url(url)),
        None => true,
    };
    if is_default {
        let url = aligned_local_url(manager);
        if url != generated_default {
            hosts
                .entries
                .insert("local".to_string(), HostEntry { urls: vec![url] });
        }
    }
    hosts
}

/// Hot-path read for request routing: no migration, no lock, no writes.
/// A missing or unreadable file yields the default registry (local only),
/// retargeted to the daemon's real `[server].port` so a non-default port
/// in config.toml doesn't strand the panel on 1820 (S5).
pub(super) fn load_hosts(manager: &ConfigManager) -> HostsConfig {
    let path = panel_settings_path(manager);
    let hosts = std::fs::read(path)
        .ok()
        .and_then(|content| {
            let settings: Result<PanelSettings, _> = serde_json::from_slice(&content);
            settings.ok().map(|settings| settings.hosts)
        })
        .unwrap_or_default();
    align_local_entry(manager, hosts)
}

pub(super) fn save_for_manager(
    manager: &ConfigManager,
    settings: &PanelSettings,
) -> Result<(), String> {
    validate(settings)?;
    save(&panel_settings_path(manager), settings)
}

fn legacy_panel_settings_path(manager: &ConfigManager) -> PathBuf {
    manager
        .app_config_path()
        .parent()
        .map(|parent| parent.join("panel-settings.json"))
        .unwrap_or_else(|| PathBuf::from("panel-settings.json"))
}

/// New home for panel settings: `panel/settings.json`. If a legacy
/// `panel-settings.json` exists and the new file does not yet, it is moved
/// into place once so subsequent reads/writes only ever touch the new path.
pub(crate) fn panel_settings_path(manager: &ConfigManager) -> PathBuf {
    let new_path = manager
        .app_config_path()
        .parent()
        .map(|parent| parent.join("panel").join("settings.json"))
        .unwrap_or_else(|| PathBuf::from("panel/settings.json"));

    if !new_path.exists() {
        let old_path = legacy_panel_settings_path(manager);
        if old_path.exists() {
            if let Some(parent) = new_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Best-effort one-time migration; on failure callers fall through
            // to creating a fresh file at new_path, old data stays in place.
            let _ = std::fs::rename(&old_path, &new_path);
        }
    }

    new_path
}

pub(super) fn normalize_language(language: &str) -> Result<String, String> {
    match language.trim() {
        "auto" => Ok("auto".to_string()),
        "zh" | "zh-cn" => Ok("zh".to_string()),
        "en" => Ok("en".to_string()),
        _ => Err("Language must be auto, zh, or en".to_string()),
    }
}

fn validate(settings: &PanelSettings) -> Result<(), String> {
    normalize_language(&settings.language).map(|_| ())
}

fn save(path: &Path, settings: &PanelSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|error| error.to_string())?;
    file.write_all(&content)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

/// One-time migration: drop `[ui].language` after panel-settings.json landed.
/// Unrelated keys inside `[ui]` and every other section are preserved.
fn remove_legacy_language(manager: &ConfigManager, mut raw: Value) -> Result<(), String> {
    let Some(object) = raw.as_object_mut() else {
        return Ok(());
    };
    let Some(ui) = object.get_mut("ui").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    if ui.remove("language").is_none() {
        return Ok(());
    }
    if ui.is_empty() {
        object.remove("ui");
    }
    if !manager.app_config_path().exists() {
        return Ok(());
    }
    let toml_value: toml::Value = serde_json::from_value(raw).map_err(|error| error.to_string())?;
    let content = toml::to_string_pretty(&toml_value).map_err(|error| error.to_string())?;
    let path = manager.app_config_path();
    let tmp = path.with_extension("toml.tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|error| error.to_string())?;
    file.write_all(content.as_bytes())
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "panel-settings-{tag}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn missing_files_yield_default_without_writing() {
        let dir = unique_dir("default");
        std::fs::create_dir_all(&dir).unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        let settings = load_or_migrate(&manager).unwrap();
        assert_eq!(settings.language, "en");
        assert!(!dir.join("panel/settings.json").exists());
        assert!(!dir.join("config.toml").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn existing_file_is_authoritative_over_legacy_ui() {
        let dir = unique_dir("authoritative");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "[ui]\nlanguage = \"zh\"\n").unwrap();
        std::fs::write(dir.join("panel-settings.json"), r#"{ "language": "en" }"#).unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        assert_eq!(load_or_migrate(&manager).unwrap().language, "en");
        // The new file is authoritative; the legacy value is never merged in.
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_file_is_an_explicit_error() {
        let dir = unique_dir("malformed");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("panel-settings.json"), "{ broken").unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        assert!(load_or_migrate(&manager).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_fields_are_ignored_but_bad_hosts_entries_fail() {
        let dir = unique_dir("unknown-fields");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("panel-settings.json"),
            r#"{ "language": "en", "theme": "dark" }"#,
        )
        .unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        // HostsConfig's flattened entries map swallows unknown top-level keys
        // (documented on PanelSettings); the file still loads.
        assert_eq!(load_or_migrate(&manager).unwrap().language, "en");

        // Strictness survives where it matters: a hosts entry that fails
        // HostEntry's untagged deserialization is an explicit error.
        std::fs::write(
            dir.join("panel/settings.json"),
            r#"{ "language": "en", "hosts": { "x": { "urls": "not-a-list" } } }"#,
        )
        .unwrap();
        assert!(load_or_migrate(&manager).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_language_in_file_is_an_explicit_error() {
        let dir = unique_dir("invalid-language");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("panel-settings.json"), r#"{ "language": "fr" }"#).unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        assert!(load_or_migrate(&manager).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_language_migrates_and_preserves_other_sections() {
        let dir = unique_dir("migrate");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[api]\nport = 1822\n\n[ui]\nlanguage = \"zh-cn\"\n\n[hosts]\nactive = \"local\"\n[hosts.local]\nurls = [\"http://127.0.0.1:1820\"]\n",
        )
        .unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        let settings = load_or_migrate(&manager).unwrap();
        assert_eq!(settings.language, "zh");

        let migrated: PanelSettings =
            serde_json::from_slice(&std::fs::read(dir.join("panel/settings.json")).unwrap())
                .unwrap();
        assert_eq!(migrated.language, "zh");

        let raw = manager.load_raw_app_config_value().unwrap();
        assert!(raw.get("ui").is_none());
        assert_eq!(raw["api"]["port"], 1822);
        assert_eq!(raw["hosts"]["active"], "local");
        assert!(!dir.join("panel/settings.json.tmp").exists());
        assert!(!dir.join("config.toml.tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_ui_keeps_unrelated_keys() {
        let dir = unique_dir("ui-extra");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[ui]\nlanguage = \"en\"\ntheme = \"dark\"\n",
        )
        .unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        load_or_migrate(&manager).unwrap();
        let raw = manager.load_raw_app_config_value().unwrap();
        assert!(raw["ui"].get("language").is_none());
        assert_eq!(raw["ui"]["theme"], "dark");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_round_trips_and_leaves_no_tmp_residue() {
        let dir = unique_dir("save");
        std::fs::create_dir_all(&dir).unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

        save_for_manager(
            &manager,
            &PanelSettings {
                language: "zh".into(),
                hosts: Default::default(),
            },
        )
        .unwrap();
        assert_eq!(load_or_migrate(&manager).unwrap().language, "zh");
        assert!(!dir.join("panel/settings.json.tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn normalizes_supported_languages() {
        assert_eq!(normalize_language("zh-cn").unwrap(), "zh");
        assert!(normalize_language("fr").is_err());
    }
}
