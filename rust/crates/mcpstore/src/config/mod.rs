use std::path::PathBuf;

mod app_schema;
mod app_validation;
mod cache_schema;
mod defaults;
mod examples;
mod field_validation;
mod flatten;
mod health_schema;
mod health_validation;
mod manager;
mod mcp_schema;
mod merge;
pub mod models;
mod monitoring_schema;
mod monitoring_validation;
pub mod resolver;
mod server_validation;
mod service_schema;
mod standalone_schema;
mod standalone_validation;
#[cfg(test)]
mod tests;
pub mod validator;

pub use app_schema::{AppConfig, ServerSettings, ServiceDefaultsConfig, UiConfig};
pub use cache_schema::{CacheBackend, CacheConfig};
pub use health_schema::HealthCheckConfig;
pub use manager::ConfigManager;
pub use mcp_schema::McpConfig;
pub use merge::merge_config;
pub use monitoring_schema::MonitoringConfig;
pub use service_schema::{
    McpStoreExtension, ResolvedServiceLifecycle, RestartPolicy, RestartPolicyKind,
    ScopeDeclarations, ScopeDescriptor, ServerConfig, ServiceLifecycleConfig,
    ServiceLifecycleDefaults, StartupPolicy,
};
pub use standalone_schema::StandaloneConfig;

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
