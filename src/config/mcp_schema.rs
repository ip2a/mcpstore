use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::service_schema::ServerConfig;
use super::{ConfigError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, ServerConfig>,
}

impl McpConfig {
    pub fn from_input_value(value: Value) -> Result<Self> {
        match value {
            Value::Object(object) if object.contains_key("mcpServers") => {
                serde_json::from_value(Value::Object(object)).map_err(Into::into)
            }
            Value::Object(mut object) => {
                let service_name = object
                    .remove("name")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        ConfigError::Invalid(
                            "service config must contain mcpServers or a non-empty name"
                                .to_string(),
                        )
                    })?;
                let server_config = serde_json::from_value(Value::Object(object))?;
                Ok(Self {
                    mcp_servers: HashMap::from([(service_name, server_config)]),
                })
            }
            Value::Array(values) => {
                let mut config = Self::default();
                for value in values {
                    for (service_name, server_config) in Self::from_input_value(value)?.mcp_servers
                    {
                        if config
                            .mcp_servers
                            .insert(service_name.clone(), server_config)
                            .is_some()
                        {
                            return Err(ConfigError::Invalid(format!(
                                "duplicate service name in config input: {service_name}"
                            )));
                        }
                    }
                }
                Ok(config)
            }
            _ => Err(ConfigError::Invalid(
                "service config must be an object or list".to_string(),
            )),
        }
    }

    pub fn from_json_str(value: &str) -> Result<Self> {
        Self::from_input_value(serde_json::from_str(value)?)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        match path.extension().and_then(|value| value.to_str()) {
            Some("json") => Self::from_json_str(&content),
            Some("toml") => {
                let value: toml::Value = toml::from_str(&content)?;
                Self::from_input_value(serde_json::to_value(value)?)
            }
            _ => Err(ConfigError::Invalid(format!(
                "Unsupported service config file format for '{}': expected .json or .toml",
                path.display()
            ))),
        }
    }

    pub fn validate_structure(&self) -> std::result::Result<(), String> {
        for (service_name, config) in &self.mcp_servers {
            if service_name.trim().is_empty() {
                return Err("mcpServers contains an empty service name".to_string());
            }
            config
                .validate_structure()
                .map_err(|error| format!("service '{service_name}': {error}"))?;
        }
        Ok(())
    }

    pub fn agent_ids(&self) -> Vec<String> {
        let mut agent_ids = self
            .mcp_servers
            .values()
            .flat_map(|config| config.scopes().agents.into_keys())
            .collect::<Vec<_>>();
        agent_ids.sort();
        agent_ids.dedup();
        agent_ids
    }

    pub fn services_for_agent(&self, agent_id: &str) -> Vec<String> {
        let mut services = self
            .mcp_servers
            .iter()
            .filter(|(_, config)| config.scopes().agents.contains_key(agent_id))
            .map(|(service_name, _)| service_name.clone())
            .collect::<Vec<_>>();
        services.sort();
        services
    }
}

#[cfg(test)]
mod tests {
    use super::McpConfig;
    use serde_json::json;

    #[test]
    fn parses_document_single_service_and_list_inputs() {
        let document = McpConfig::from_input_value(json!({
            "mcpServers": {"document": {"url": "http://example.test/mcp"}}
        }))
        .unwrap();
        assert!(document.mcp_servers.contains_key("document"));

        let single = McpConfig::from_input_value(json!({
            "name": "single",
            "command": "echo",
            "args": ["ok"]
        }))
        .unwrap();
        assert!(single.mcp_servers.contains_key("single"));

        let list = McpConfig::from_input_value(json!([
            {"name": "first", "command": "echo"},
            {"name": "second", "url": "http://example.test/mcp"}
        ]))
        .unwrap();
        assert_eq!(list.mcp_servers.len(), 2);
        assert!(list.mcp_servers.contains_key("first"));
        assert!(list.mcp_servers.contains_key("second"));
    }

    #[test]
    fn parses_json_and_toml_files() {
        let directory =
            std::env::temp_dir().join(format!("mcpstore-config-input-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let json_path = directory.join("services.json");
        let toml_path = directory.join("services.toml");
        std::fs::write(&json_path, r#"{"mcpServers":{"json":{"command":"echo"}}}"#).unwrap();
        std::fs::write(&toml_path, "[mcpServers.toml]\ncommand = \"echo\"\n").unwrap();

        assert!(McpConfig::from_file(&json_path)
            .unwrap()
            .mcp_servers
            .contains_key("json"));
        assert!(McpConfig::from_file(&toml_path)
            .unwrap()
            .mcp_servers
            .contains_key("toml"));

        std::fs::remove_dir_all(directory).ok();
    }
}
