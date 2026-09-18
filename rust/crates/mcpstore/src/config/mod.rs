use std::path::PathBuf;

mod api_web_validation;
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
pub mod resolver;
mod service_schema;
#[cfg(test)]
mod tests;
pub mod validator;

pub use crate::auth::{
    AuthConfig, AuthorizationCodeClientAuthMethod, ClientCredentialsAuthMethod,
    OAuthAuthorizationCodeConfig, OAuthClientCredentialsConfig,
};
pub use app_schema::{
    ApiSettings, AppConfig, DiagnosticsConfig, HostEntry, HostsConfig, McpAggregateConfig,
    RuntimeLogConfig, ServiceDefaultsConfig, UiConfig, WebSettings,
};
pub use cache_schema::CacheConfig;
pub use health_schema::HealthCheckConfig;
pub use manager::ConfigManager;
pub use mcp_schema::McpConfig;
pub use merge::merge_config;
pub use service_schema::{
    HandshakeMode, McpStoreExtension, ResolvedServiceLifecycle, RestartPolicy, RestartPolicyKind,
    ScopeDeclarations, ScopeDescriptor, ServerConfig, ServiceLifecycleConfig,
    ServiceLifecycleDefaults, StartupPolicy,
};

pub const DEFAULT_RUNTIME_LOG_LEVEL: &str = "info";
pub const DEFAULT_API_URL_PREFIX: &str = "";

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
