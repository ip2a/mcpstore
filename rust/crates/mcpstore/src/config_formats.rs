use crate::config::McpConfig;
use crate::{Result, StoreError};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Native,
    Claude,
    Codex,
}

impl FromStr for ConfigFormat {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "native" => Ok(Self::Native),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            other => Err(StoreError::Other(format!(
                "Unsupported config format: {other}"
            ))),
        }
    }
}

pub fn project_config(config: &McpConfig, format: ConfigFormat) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(config)
        .map_err(|error| StoreError::Other(format!("Config projection failed: {error}")))?;
    if !matches!(format, ConfigFormat::Native) {
        strip_mcpstore_extension(&mut value);
    }
    Ok(value)
}

fn strip_mcpstore_extension(value: &mut serde_json::Value) {
    let Some(servers) = value
        .get_mut("mcpServers")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    for server in servers.values_mut() {
        if let Some(server) = server.as_object_mut() {
            server.remove("_mcpstore");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        McpStoreExtension, RestartPolicy, RestartPolicyKind, ServerConfig, ServiceLifecycleConfig,
        StartupPolicy,
    };
    use std::collections::HashMap;

    fn service_with_lifecycle() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("node".to_string()),
            args: vec!["server.js".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: None,
            mcpstore: Some(McpStoreExtension {
                lifecycle: Some(ServiceLifecycleConfig {
                    startup_policy: Some(StartupPolicy::OnStoreStart),
                    restart_policy: Some(RestartPolicy {
                        kind: RestartPolicyKind::OnFailure,
                        max_retries: Some(3),
                    }),
                }),
            }),
        }
    }

    #[test]
    fn native_projection_preserves_mcpstore_extension() {
        let mut config = McpConfig::default();
        config
            .mcp_servers
            .insert("demo".to_string(), service_with_lifecycle());

        let projected = project_config(&config, ConfigFormat::Native).unwrap();

        assert!(projected["mcpServers"]["demo"].get("_mcpstore").is_some());
    }

    #[test]
    fn third_party_projection_strips_mcpstore_extension() {
        let mut config = McpConfig::default();
        config
            .mcp_servers
            .insert("demo".to_string(), service_with_lifecycle());

        let projected = project_config(&config, ConfigFormat::Claude).unwrap();

        assert!(projected["mcpServers"]["demo"].get("_mcpstore").is_none());
        assert_eq!(
            projected["mcpServers"]["demo"]["command"],
            serde_json::json!("node")
        );
    }
}
