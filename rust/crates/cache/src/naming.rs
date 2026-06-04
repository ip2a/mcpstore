//! Naming service migrated from Python core/cache/naming_service.py
//!
//! Handles global name generation and parsing for services and tools,
//! implementing the dual-perspective naming (agent vs store view).

const GLOBAL_AGENT_STORE: &str = "global_agent_store";
const AGENT_SEPARATOR: &str = "_byagent_";

/// Generate a globally unique service name.
///
/// Rules:
/// - If agent_id is "global_agent_store", returns original_name
/// - Otherwise returns "{original_name}_byagent_{agent_id}"
pub fn generate_service_global_name(original_name: &str, agent_id: &str) -> String {
    if original_name.is_empty() {
        panic!("Service original name cannot be empty");
    }
    if agent_id.is_empty() {
        panic!("Agent ID cannot be empty");
    }

    if agent_id == GLOBAL_AGENT_STORE {
        original_name.to_string()
    } else {
        format!("{original_name}{AGENT_SEPARATOR}{agent_id}")
    }
}

/// Generate a globally unique tool name.
///
/// Rules:
/// - If tool_original_name already starts with "{service_global_name}_", return as-is
/// - Otherwise returns "{service_global_name}_{tool_original_name}"
pub fn generate_tool_global_name(service_global_name: &str, tool_original_name: &str) -> String {
    if service_global_name.is_empty() {
        panic!("Service global name cannot be empty");
    }
    if tool_original_name.is_empty() {
        panic!("Tool original name cannot be empty");
    }

    let prefix = format!("{service_global_name}_");
    if tool_original_name.starts_with(&prefix) {
        tool_original_name.to_string()
    } else {
        format!("{service_global_name}_{tool_original_name}")
    }
}

/// Parse a service global name into (original_name, agent_id).
///
/// Rules:
/// - If no "_byagent_" separator, returns (global_name, "global_agent_store")
/// - Otherwise splits from the right once
pub fn parse_service_global_name(global_name: &str) -> (String, String) {
    if global_name.is_empty() {
        panic!("Service global name cannot be empty");
    }

    if !global_name.contains(AGENT_SEPARATOR) {
        (global_name.to_string(), GLOBAL_AGENT_STORE.to_string())
    } else {
        let parts: Vec<&str> = global_name.rsplitn(2, AGENT_SEPARATOR).collect();
        if parts.len() != 2 {
            panic!(
                "Invalid service global name format: {global_name}. \
                 Expected format: 'name{AGENT_SEPARATOR}agent_id' or 'name'"
            );
        }
        (parts[1].to_string(), parts[0].to_string())
    }
}

/// Check if the agent is the global store.
pub fn is_global_agent_service(agent_id: &str) -> bool {
    agent_id == GLOBAL_AGENT_STORE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_service_global_name() {
        assert_eq!(
            generate_service_global_name("context7", "agent1"),
            "context7_byagent_agent1"
        );
        assert_eq!(
            generate_service_global_name("context7", GLOBAL_AGENT_STORE),
            "context7"
        );
    }

    #[test]
    fn test_generate_tool_global_name() {
        assert_eq!(
            generate_tool_global_name("context7_byagent_agent1", "resolve-library-id"),
            "context7_byagent_agent1_resolve-library-id"
        );
        assert_eq!(
            generate_tool_global_name("context7", "resolve-library-id"),
            "context7_resolve-library-id"
        );
        assert_eq!(
            generate_tool_global_name("weather_service", "weather_service_get_current_weather"),
            "weather_service_get_current_weather"
        );
    }

    #[test]
    fn test_parse_service_global_name() {
        assert_eq!(
            parse_service_global_name("context7_byagent_agent1"),
            ("context7".to_string(), "agent1".to_string())
        );
        assert_eq!(
            parse_service_global_name("context7"),
            ("context7".to_string(), GLOBAL_AGENT_STORE.to_string())
        );
    }

    #[test]
    fn test_is_global_agent_service() {
        assert!(is_global_agent_service(GLOBAL_AGENT_STORE));
        assert!(!is_global_agent_service("agent1"));
    }
}
