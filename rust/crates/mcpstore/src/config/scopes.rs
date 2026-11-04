//! Scope configuration (scopes.toml) - defines how services/tools/prompts/resources
//! are used in different scopes (store, agents).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Root scope configuration from scopes.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScopesConfig {
    #[serde(default)]
    pub store: ScopeConfig,
    #[serde(default)]
    pub agents: HashMap<String, ScopeConfig>,
}

/// Configuration for a single scope (store or agent)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScopeConfig {
    #[serde(default)]
    pub services: HashMap<String, ServiceScopeConfig>,
}

/// Service-level scope configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceScopeConfig {
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
    #[serde(default)]
    pub tools: ToolsScopeConfig,
    #[serde(default)]
    pub prompts: ComponentsScopeConfig,
    #[serde(default)]
    pub resources: ComponentsScopeConfig,
}

/// Tools-specific configuration (has policy allowlist)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsScopeConfig {
    /// Allowlist of tool names. None = all available, Some([]) = none available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<Vec<String>>,
    #[serde(default)]
    pub overrides: HashMap<String, ComponentOverride>,
}

/// Prompts/Resources configuration (no policy, only overrides)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentsScopeConfig {
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

impl ScopesConfig {
    /// Load from scopes.toml file
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, ScopesConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ScopesConfigError::IoError(path.as_ref().to_path_buf(), e))?;
        Self::parse_toml(&content)
    }

    /// Parse from TOML string
    pub fn parse_toml(content: &str) -> Result<Self, ScopesConfigError> {
        toml::from_str(content).map_err(ScopesConfigError::ParseError)
    }

    /// Get default scopes.toml path (~/.mcpstore/scopes.toml)
    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".mcpstore").join("scopes.toml"))
    }

    /// Load from default path if exists, otherwise empty config
    pub fn load_or_default() -> Self {
        Self::default_path()
            .and_then(|path| {
                if path.exists() {
                    Self::load_from_file(path).ok()
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScopesConfigError {
    #[error("Failed to read config file {0}: {1}")]
    IoError(PathBuf, std::io::Error),
    #[error("Failed to parse TOML: {0}")]
    ParseError(toml::de::Error),
}
