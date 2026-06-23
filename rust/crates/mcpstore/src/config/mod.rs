#[cfg(test)]
use serde_json::Value;
use std::path::PathBuf;

mod app_validation;
mod defaults;
mod examples;
mod field_validation;
mod flatten;
mod health_validation;
mod manager;
pub mod models;
mod monitoring_validation;
pub mod resolver;
mod schema;
mod server_validation;
mod standalone_validation;
pub mod validator;

#[cfg(test)]
use examples::default_server_config;
pub use manager::ConfigManager;
pub use schema::{
    AppConfig, CacheBackend, CacheConfig, HealthCheckConfig, McpConfig, MonitoringConfig,
    ServerConfig, ServerSettings, StandaloneConfig, UiConfig,
};

pub const DEFAULT_SERVER_LOG_LEVEL: &str = "info";
pub const DEFAULT_SERVER_URL_PREFIX: &str = "";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),
    #[error("JSON processing failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML parse failed: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML serialization failed: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid config: {0}")]
    Invalid(String),
    #[error("Config validation failed: {0:?}")]
    Validation(Vec<validator::ValidationError>),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mcp.json");

        let mgr = ConfigManager::with_path(&path);
        let mut config = McpConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            ServerConfig {
                url: Some("http://localhost:8080/mcp".to_string()),
                transport: Some("streamable-http".to_string()),
                ..default_server_config()
            },
        );

        mgr.save(&config).unwrap();
        let loaded = mgr.load().unwrap();
        assert_eq!(loaded.mcp_servers.len(), 1);
        assert!(loaded.mcp_servers.contains_key("test"));
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("cache"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_app_config_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));

        let mut config = AppConfig::default();
        config.cache.backend = CacheBackend::Redis;
        config.cache.redis_url = Some("redis://127.0.0.1/".to_string());
        config.server.log_level = "debug".to_string();
        config.server.url_prefix = "/demo".to_string();

        mgr.save_app_config(&config).unwrap();
        let loaded = mgr.load_app_config().unwrap();
        assert_eq!(loaded.cache.backend, CacheBackend::Redis);
        assert_eq!(loaded.cache.namespace, "mcpstore");
        assert_eq!(loaded.server.log_level, "debug");
        assert_eq!(loaded.server.url_prefix, "/demo");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_default_template_contains_runtime_sections() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));
        let template = mgr.default_app_config_toml().unwrap();

        assert!(template.contains("[server]"));
        assert!(template.contains("[health_check]"));
        assert!(template.contains("[monitoring]"));
        assert!(template.contains("[standalone]"));
        assert!(template.contains("[ui]"));
        assert!(template.contains("language = \"zh-cn\""));
        assert!(template.contains("log_level = \"info\""));
    }

    #[test]
    fn test_flatten_raw_app_config_only_contains_explicit_keys() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));
        std::fs::write(
            mgr.app_config_path(),
            "[server]\nhost = \"127.0.0.1\"\n[health_check]\nstartup_interval = 2.5\n",
        )
        .unwrap();

        let flattened = mgr.flatten_raw_app_config().unwrap();
        assert_eq!(
            flattened.get("server.host"),
            Some(&Value::String("127.0.0.1".to_string()))
        );
        assert_eq!(
            flattened.get("health_check.startup_interval"),
            Some(&serde_json::json!(2.5))
        );
        assert!(!flattened.contains_key("server.port"));
    }

    #[test]
    fn test_invalid_app_config_fails_fast() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));
        std::fs::write(
            mgr.app_config_path(),
            "[health_check]\nstartup_interval = 0.0\n",
        )
        .unwrap();

        let err = mgr.load_app_config().unwrap_err();
        assert!(err.to_string().contains("health_check.startup_interval"));
    }

    #[test]
    fn test_init_splits_mcp_json_and_app_toml() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        let mgr = ConfigManager::with_path(dir.join("mcp.json"));

        mgr.init(true, Some("redis://127.0.0.1/".to_string()))
            .unwrap();

        let mcp_raw = std::fs::read_to_string(mgr.mcp_path()).unwrap();
        assert!(mcp_raw.contains("mcpServers"));
        assert!(!mcp_raw.contains("cache"));

        let app_config = mgr.load_app_config().unwrap();
        assert_eq!(app_config.cache.backend, CacheBackend::Redis);
        assert_eq!(
            app_config.cache.redis_url.as_deref(),
            Some("redis://127.0.0.1/")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_infer_transport() {
        let http = ServerConfig {
            url: Some("http://x".into()),
            ..default_server_config()
        };
        assert_eq!(http.infer_transport(), "streamable-http");

        let stdio = ServerConfig {
            command: Some("python".into()),
            ..default_server_config()
        };
        assert_eq!(stdio.infer_transport(), "stdio");
    }

    #[test]
    fn test_add_examples_only_inserts_missing_entries() {
        let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
        let path = dir.join("mcp.json");
        let mgr = ConfigManager::with_path(&path);

        let mut config = McpConfig::default();
        config.mcp_servers.insert(
            "remote-http-service".to_string(),
            ServerConfig {
                url: Some("https://override.example.com/mcp".to_string()),
                transport: Some("streamable-http".to_string()),
                description: Some("Existing service".to_string()),
                ..default_server_config()
            },
        );
        config.mcp_servers.insert(
            "custom-service".to_string(),
            ServerConfig {
                command: Some("echo".to_string()),
                args: vec!["ok".to_string()],
                transport: Some("stdio".to_string()),
                ..default_server_config()
            },
        );

        mgr.save(&config).unwrap();
        let added = mgr.add_examples().unwrap();
        let loaded = mgr.load().unwrap();

        assert_eq!(added, 2);
        assert_eq!(loaded.mcp_servers.len(), 4);
        assert_eq!(
            loaded
                .mcp_servers
                .get("remote-http-service")
                .and_then(|svc| svc.url.as_deref()),
            Some("https://override.example.com/mcp")
        );
        assert!(loaded.mcp_servers.contains_key("local-command-service"));
        assert!(loaded.mcp_servers.contains_key("npm-package-service"));
        assert!(loaded.mcp_servers.contains_key("custom-service"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
