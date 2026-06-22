mod names;
mod tools;

pub use names::{
    generate_service_global_name, generate_tool_global_name, normalize_service_name,
    parse_agent_scoped, AgentScopedName, ServiceResolution, AGENT_SEPARATOR, GLOBAL_AGENT_STORE,
};
pub use tools::{resolve_tool, AvailableTool, ToolResolution};

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<AvailableTool> {
        vec![
            AvailableTool {
                name: "weather_get_current_weather".to_string(),
                original_name: Some("get_current_weather".to_string()),
                service_name: Some("weather".to_string()),
                global_service_name: Some("weather_byagent_agent1".to_string()),
                global_tool_name: None,
            },
            AvailableTool {
                name: "docs_get_page".to_string(),
                original_name: Some("get_page".to_string()),
                service_name: Some("docs".to_string()),
                global_service_name: Some("docs_byagent_agent1".to_string()),
                global_tool_name: None,
            },
        ]
    }

    #[test]
    fn normalizes_agent_service_names() {
        let parsed = parse_agent_scoped("agent1:weather").unwrap();
        assert_eq!(parsed.agent_id.as_deref(), Some("agent1"));
        assert_eq!(parsed.local_name, "weather");

        let parsed = parse_agent_scoped("weather_byagent_shadow_byagent_agent1").unwrap();
        assert_eq!(parsed.agent_id.as_deref(), Some("agent1"));
        assert_eq!(parsed.local_name, "weather_byagent_shadow");

        let normalized = normalize_service_name("agent1", "weather", "global", true).unwrap();
        assert_eq!(normalized.global_name, "weather_byagent_agent1");
        assert_eq!(normalized.local_name, "weather");
    }

    #[test]
    fn resolves_prefixed_tool_to_canonical() {
        let resolved = resolve_tool(
            "agent1",
            "weather_get_current_weather",
            &tools(),
            "canonical",
            true,
        )
        .unwrap();
        assert_eq!(resolved.canonical_tool_name, "get_current_weather");
        assert_eq!(
            resolved.global_tool_name,
            "weather_byagent_agent1_get_current_weather"
        );
    }

    #[test]
    fn rejects_ambiguous_no_prefix_tool() {
        let mut tools = tools();
        tools.push(AvailableTool {
            name: "another_get_current_weather".to_string(),
            original_name: Some("get_current_weather".to_string()),
            service_name: Some("another".to_string()),
            global_service_name: None,
            global_tool_name: None,
        });
        let err = resolve_tool("agent1", "get_current_weather", &tools, "canonical", true)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Ambiguous"));
    }
}
