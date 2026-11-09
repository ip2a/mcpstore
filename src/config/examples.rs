use std::collections::HashMap;

use super::ServerConfig;

pub(super) fn default_server_config() -> ServerConfig {
    ServerConfig::default()
}

pub(super) fn example_services() -> HashMap<String, ServerConfig> {
    let mut services = HashMap::new();
    services.insert(
        "remote-http-service".to_string(),
        ServerConfig {
            url: Some("https://example.com/mcp".to_string()),
            transport: Some("streamable-http".to_string()),
            description: Some("Example remote HTTP MCP service".to_string()),
            ..default_server_config()
        },
    );
    services.insert(
        "local-command-service".to_string(),
        ServerConfig {
            command: Some("python".to_string()),
            args: vec!["-m".to_string(), "your_mcp_server".to_string()],
            env: HashMap::new(),
            working_dir: Some(".".to_string()),
            description: Some("Example local command MCP service".to_string()),
            ..default_server_config()
        },
    );
    services.insert(
        "npm-package-service".to_string(),
        ServerConfig {
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "some-mcp-package".to_string()],
            transport: Some("stdio".to_string()),
            description: Some("Example NPM package MCP service".to_string()),
            ..default_server_config()
        },
    );
    services
}
