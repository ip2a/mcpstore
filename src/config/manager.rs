use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::app_validation::validate_app_config;
use super::examples::example_services;
use super::flatten::flatten_config_value;
use super::resolver;
use super::{AppConfig, ConfigError, KvConfig, McpConfig, Result};

pub struct ConfigManager {
    mcp_path: PathBuf,
    app_config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let mcp_path = resolver::resolve_mcp_config_path();
        Self {
            // Default app config is fixed under the user data dir (device state),
            // not colocated with mcp.json — only explicit with_path() colocates.
            app_config_path: resolver::app_config_path(),
            mcp_path,
        }
    }

    pub fn with_path(path: impl AsRef<Path>) -> Self {
        let mcp_path = path.as_ref().to_path_buf();
        Self {
            app_config_path: resolver::app_config_path_for_mcp_path(&mcp_path),
            mcp_path,
        }
    }

    pub fn path(&self) -> &Path {
        self.mcp_path()
    }

    pub fn mcp_path(&self) -> &Path {
        &self.mcp_path
    }

    pub fn app_config_path(&self) -> &Path {
        &self.app_config_path
    }

    pub fn exists(&self) -> bool {
        self.mcp_path.exists()
    }

    pub fn app_config_exists(&self) -> bool {
        self.app_config_path.exists()
    }

    pub fn load(&self) -> Result<McpConfig> {
        if !self.mcp_path.exists() {
            return Err(ConfigError::NotFound(self.mcp_path.clone()));
        }
        let content = std::fs::read_to_string(&self.mcp_path)?;
        let config: McpConfig = serde_json::from_str(&content)?;
        config.validate_structure().map_err(ConfigError::Invalid)?;
        Ok(config)
    }

    pub fn load_or_empty(&self) -> Result<McpConfig> {
        match self.load() {
            Ok(config) => Ok(config),
            Err(ConfigError::NotFound(_)) => Ok(McpConfig::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, config: &McpConfig) -> Result<()> {
        config.validate_structure().map_err(ConfigError::Invalid)?;
        let mut config = config.clone();
        for server in config.mcp_servers.values_mut() {
            server.ensure_native_scopes();
        }
        if let Some(parent) = self.mcp_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&self.mcp_path, content)?;
        Ok(())
    }

    pub fn load_app_config(&self) -> Result<AppConfig> {
        if !self.app_config_path.exists() {
            return Err(ConfigError::NotFound(self.app_config_path.clone()));
        }
        let content = std::fs::read_to_string(&self.app_config_path)?;
        let raw: toml::Value = toml::from_str(&content)?;
        // Hard cut: [cache] was renamed to [kv]. AppConfig deliberately tolerates
        // foreign sections (carry_over_foreign_sections), so serde alone would
        // silently ignore the old name — reject it loudly instead.
        if raw.get("cache").is_some() {
            return Err(ConfigError::Invalid(
                "config section [cache] was renamed to [kv]; rename the section in your config file"
                    .to_string(),
            ));
        }
        let config: AppConfig = raw.try_into()?;
        validate_app_config(&config)?;
        Ok(config)
    }

    pub fn load_app_config_or_default(&self) -> Result<AppConfig> {
        match self.load_app_config() {
            Ok(config) => Ok(config),
            Err(ConfigError::NotFound(_)) => Ok(AppConfig::default()),
            Err(err) => Err(err),
        }
    }

    pub fn save_app_config(&self, config: &AppConfig) -> Result<()> {
        validate_app_config(config)?;

        // Compute sparse representation: only write fields that differ from defaults
        let sparse_value =
            self.carry_over_foreign_sections(super::sparse::diff_from_default(config)?)?;

        if let Some(parent) = self.app_config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Serialize sparse value as TOML
        let toml_value: toml::Value = serde_json::from_value(sparse_value)?;
        let content = toml::to_string_pretty(&toml_value)?;

        // Flush must be atomic (temp file + rename) so a mid-write crash can't corrupt the config file.
        let tmp = self.app_config_path.with_extension("toml.tmp");
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &self.app_config_path)?;
        Ok(())
    }

    /// A save rewrites the sections AppConfig owns; top-level sections the
    /// schema no longer knows (e.g. a `[ui]` pending panel-settings migration)
    /// are carried over untouched instead of being silently dropped. Known
    /// sections are replaced wholesale, so reverting a field to its default
    /// still disappears from the file.
    fn carry_over_foreign_sections(&self, sparse: Value) -> Result<Value> {
        let known_keys: std::collections::HashSet<String> =
            serde_json::to_value(AppConfig::default())?
                .as_object()
                .map(|object| object.keys().cloned().collect())
                .unwrap_or_default();
        let mut merged = sparse.as_object().cloned().unwrap_or_default();
        if self.app_config_path.exists() {
            let raw = self.load_raw_app_config_value()?;
            if let Some(raw_object) = raw.as_object() {
                for (key, value) in raw_object {
                    if !known_keys.contains(key) && !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        Ok(Value::Object(merged))
    }

    pub fn default_app_config_toml(&self) -> Result<String> {
        toml::to_string_pretty(&AppConfig::default()).map_err(Into::into)
    }

    pub fn load_raw_app_config_value(&self) -> Result<Value> {
        if !self.app_config_path.exists() {
            return Ok(Value::Object(Map::new()));
        }
        let content = std::fs::read_to_string(&self.app_config_path)?;
        let value: toml::Value = toml::from_str(&content)?;
        serde_json::to_value(value).map_err(Into::into)
    }

    pub fn flatten_raw_app_config(&self) -> Result<HashMap<String, Value>> {
        let value = self.load_raw_app_config_value()?;
        Ok(flatten_config_value(&value))
    }

    pub fn validate(&self) -> Result<()> {
        let config = self.load()?;
        config.validate_structure().map_err(ConfigError::Invalid)?;
        self.validate_app_config()?;
        Ok(())
    }

    pub fn validate_app_config(&self) -> Result<()> {
        self.load_app_config_or_default().map(|_| ())
    }

    pub fn init(&self, with_examples: bool, redis_url: Option<String>) -> Result<()> {
        let mut config = McpConfig::default();
        let mut app_config = AppConfig::default();
        if let Some(redis_url) = redis_url {
            app_config.kv = KvConfig {
                store: "redis".to_string(),
                config: serde_json::json!({"url": redis_url}),
                namespace: "mcpstore".to_string(),
            };
        }
        if with_examples {
            config.mcp_servers.extend(example_services());
        }
        self.save(&config)?;
        self.save_app_config(&app_config)
    }

    pub fn add_examples(&self) -> Result<usize> {
        let mut config = self.load_or_empty()?;
        let mut added = 0usize;

        for (name, service) in example_services() {
            if config.mcp_servers.contains_key(&name) {
                continue;
            }
            config.mcp_servers.insert(name, service);
            added += 1;
        }

        self.save(&config)?;
        Ok(added)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
