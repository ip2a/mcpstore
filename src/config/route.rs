//! Gateway route table (`route.toml`) — controls how services/tools/prompts/resources
//! are projected on the gateway (rename, rewrite metadata, hide, arg remap, tool policy).
//!
//! This is intentionally separate from `scopes.toml`, which is reserved for scope
//! membership concerns. KV remains the runtime source of truth; `route.toml` is the
//! declarative file mirror loaded on boot/reload and written back from the panel.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Root route table from route.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouteConfig {
    #[serde(default)]
    pub store: RouteScopeConfig,
    #[serde(default)]
    pub agents: HashMap<String, RouteScopeConfig>,
}

/// Route rules for one scope (store or agent)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouteScopeConfig {
    #[serde(default)]
    pub services: HashMap<String, ServiceRouteConfig>,
}

/// Per-service gateway projection rules
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceRouteConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<IconsOverride>,
    #[serde(default)]
    pub tools: ToolsRouteConfig,
    #[serde(default)]
    pub prompts: ComponentsRouteConfig,
    #[serde(default)]
    pub resources: ComponentsRouteConfig,
}

/// Tools-specific route rules (has policy allowlist)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsRouteConfig {
    /// Allowlist of tool names. None = all available, Some([]) = none available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<Vec<String>>,
    #[serde(default)]
    pub overrides: HashMap<String, ComponentOverride>,
}

/// Prompts/Resources route rules (no policy, only overrides)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentsRouteConfig {
    #[serde(default)]
    pub overrides: HashMap<String, ComponentOverride>,
}

/// Override for a single component (tool/prompt/resource)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<HashMap<String, ArgumentOverride>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<IconsOverride>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconsOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark: Option<String>,
}

impl RouteConfig {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, RouteConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| RouteConfigError::IoError(path.as_ref().to_path_buf(), e))?;
        Self::parse_toml(&content)
    }

    pub fn parse_toml(content: &str) -> Result<Self, RouteConfigError> {
        toml::from_str(content).map_err(RouteConfigError::ParseError)
    }

    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".mcpstore").join("route.toml"))
    }

    /// Load from an explicit path. `Ok(None)` when the file does not exist;
    /// parse and IO errors propagate — a broken route.toml is never silently
    /// treated as an empty config.
    pub fn load_if_exists(path: impl AsRef<Path>) -> Result<Option<Self>, RouteConfigError> {
        let path = path.as_ref();
        match std::fs::read_to_string(path) {
            Ok(content) => Self::parse_toml(&content).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(RouteConfigError::IoError(path.to_path_buf(), error)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RouteConfigError {
    #[error("Failed to read config file {0}: {1}")]
    IoError(PathBuf, std::io::Error),
    #[error("Failed to parse TOML: {0}")]
    ParseError(toml::de::Error),
}

/// Temporary aliases while call sites migrate off the scopes.toml naming.
pub type ScopesConfig = RouteConfig;
pub type ScopesConfigError = RouteConfigError;
pub type ScopeConfig = RouteScopeConfig;
pub type ServiceScopeConfig = ServiceRouteConfig;
pub type ToolsScopeConfig = ToolsRouteConfig;
pub type ComponentsScopeConfig = ComponentsRouteConfig;
