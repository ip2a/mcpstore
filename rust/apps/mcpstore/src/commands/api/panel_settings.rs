use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use mcpstore::config::ConfigManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PanelSettings {
    pub language: String,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
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
    if path.exists() {
        let settings: PanelSettings =
            serde_json::from_slice(&std::fs::read(&path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        validate(&settings)?;
        return Ok(settings);
    }

    let raw = manager
        .load_raw_app_config_value()
        .map_err(|error| error.to_string())?;
    let Some(language) = raw
        .get("ui")
        .and_then(|ui| ui.get("language"))
        .and_then(Value::as_str)
    else {
        return Ok(PanelSettings::default());
    };
    let settings = PanelSettings {
        language: normalize_language(language)?,
    };
    save(&path, &settings)?;
    remove_legacy_language(manager, raw)?;
    Ok(settings)
}

pub(super) fn save_for_manager(
    manager: &ConfigManager,
    settings: &PanelSettings,
) -> Result<(), String> {
    validate(settings)?;
    save(&panel_settings_path(manager), settings)
}

fn panel_settings_path(manager: &ConfigManager) -> PathBuf {
    manager
        .app_config_path()
        .parent()
        .map(|parent| parent.join("panel-settings.json"))
        .unwrap_or_else(|| PathBuf::from("panel-settings.json"))
}

pub(super) fn normalize_language(language: &str) -> Result<String, String> {
    match language.trim() {
        "auto" => Ok("auto".to_string()),
        "zh" | "zh-cn" => Ok("zh".to_string()),
        "en" => Ok("en".to_string()),
        _ => Err("语言必须是 auto、zh 或 en".to_string()),
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
        assert!(!dir.join("panel-settings.json").exists());
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
    fn unknown_fields_are_rejected() {
        let dir = unique_dir("unknown-fields");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("panel-settings.json"),
            r#"{ "language": "en", "theme": "dark" }"#,
        )
        .unwrap();
        let manager = ConfigManager::with_path(dir.join("mcp.json"));

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
            serde_json::from_slice(&std::fs::read(dir.join("panel-settings.json")).unwrap())
                .unwrap();
        assert_eq!(migrated.language, "zh");

        let raw = manager.load_raw_app_config_value().unwrap();
        assert!(raw.get("ui").is_none());
        assert_eq!(raw["api"]["port"], 1822);
        assert_eq!(raw["hosts"]["active"], "local");
        assert!(!dir.join("panel-settings.json.tmp").exists());
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
            },
        )
        .unwrap();
        assert_eq!(load_or_migrate(&manager).unwrap().language, "zh");
        assert!(!dir.join("panel-settings.json.tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn normalizes_supported_languages() {
        assert_eq!(normalize_language("zh-cn").unwrap(), "zh");
        assert!(normalize_language("fr").is_err());
    }
}
