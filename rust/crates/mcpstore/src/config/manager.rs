use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::app_validation::validate_app_config;
use super::examples::example_services;
use super::flatten::flatten_config_value;
use super::{models, resolver, validator};
use super::{AppConfig, CacheBackend, CacheConfig, ConfigError, McpConfig, Result};

pub struct ConfigManager {
    mcp_path: PathBuf,
    app_config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let mcp_path = resolver::resolve_mcp_config_path();
        Self {
            app_config_path: resolver::app_config_path_for_mcp_path(&mcp_path),
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
        Ok(config)
    }

    pub fn load_or_default(&self) -> McpConfig {
        self.load().unwrap_or_default()
    }

    pub fn save(&self, config: &McpConfig) -> Result<()> {
        if let Some(parent) = self.mcp_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&self.mcp_path, content)?;
        Ok(())
    }

    pub fn load_app_config(&self) -> Result<AppConfig> {
        if !self.app_config_path.exists() {
            return Err(ConfigError::NotFound(self.app_config_path.clone()));
        }
        let content = std::fs::read_to_string(&self.app_config_path)?;
        let config: AppConfig = toml::from_str(&content)?;
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
        if let Some(parent) = self.app_config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(config)?;
        std::fs::write(&self.app_config_path, content)?;
        Ok(())
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
        let servers: HashMap<String, models::ServerConfigFull> = config
            .mcp_servers
            .into_iter()
            .map(|(k, v)| {
                let full = models::ServerConfigFull {
                    url: v.url,
                    command: v.command,
                    args: v.args,
                    env: v.env,
                    headers: v.headers,
                    transport: v
                        .transport
                        .and_then(|t| serde_json::from_value(serde_json::Value::String(t)).ok()),
                    working_dir: v.working_dir,
                };
                (k, full)
            })
            .collect();
        validator::validate(&servers).map_err(ConfigError::Validation)?;
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
            app_config.cache = CacheConfig {
                backend: CacheBackend::Redis,
                redis_url: Some(redis_url),
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
        let mut config = self.load_or_default();
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
