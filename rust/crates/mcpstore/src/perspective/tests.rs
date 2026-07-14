use super::*;
use crate::{ScopeRef, ServiceInstanceKey};

fn available_tool(
    service_name: &str,
    scope: ScopeRef,
    tool_name: &str,
    name: &str,
) -> AvailableTool {
    let instance_id = ServiceInstanceKey::new(service_name, scope.clone()).instance_id();
    AvailableTool {
        instance_id,
        service_name: service_name.to_string(),
        scope,
        tool_name: tool_name.to_string(),
        name: name.to_string(),
    }
}

fn tools() -> Vec<AvailableTool> {
    vec![
        available_tool(
            "weather",
            ScopeRef::Agent {
                agent_id: "agent1".to_string(),
            },
            "get_current_weather",
            "weather_get_current_weather",
        ),
        available_tool("docs", ScopeRef::Store, "get_page", "docs_get_page"),
    ]
}

#[test]
fn resolves_display_name_to_explicit_instance() {
    let available = tools();
    let resolved = resolve_tool("weather_get_current_weather", &available).unwrap();
    assert_eq!(resolved.instance_id, available[0].instance_id);
    assert_eq!(resolved.service_name, "weather");
    assert_eq!(
        resolved.scope,
        ScopeRef::Agent {
            agent_id: "agent1".to_string()
        }
    );
    assert_eq!(resolved.tool_name, "get_current_weather");
    assert_eq!(resolved.resolution_method, "exact_match");
}

#[test]
fn rejects_same_tool_across_isolated_scopes_as_ambiguous() {
    let available = vec![
        available_tool(
            "weather",
            ScopeRef::Store,
            "get_current_weather",
            "get_current_weather",
        ),
        available_tool(
            "weather",
            ScopeRef::Agent {
                agent_id: "agent1".to_string(),
            },
            "get_current_weather",
            "get_current_weather",
        ),
    ];
    let err = resolve_tool("get_current_weather", &available)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Ambiguous"));
}

#[test]
fn does_not_infer_store_or_sibling_scope() {
    let available = vec![available_tool(
        "weather",
        ScopeRef::Agent {
            agent_id: "agent2".to_string(),
        },
        "forecast",
        "agent2_forecast",
    )];

    let resolved = resolve_tool("agent2_forecast", &available).unwrap();
    assert_eq!(resolved.instance_id, available[0].instance_id);
    assert_eq!(resolved.scope, available[0].scope);
}

#[test]
fn rejects_inconsistent_instance_identity() {
    let mut available = tools();
    available[0].instance_id =
        ServiceInstanceKey::new("other", available[0].scope.clone()).instance_id();

    let err = resolve_tool("weather_get_current_weather", &available)
        .unwrap_err()
        .to_string();
    assert!(err.contains("does not match"));
}

#[test]
fn resolution_serialization_contains_only_explicit_identity() {
    let available = tools();
    let resolution = resolve_tool("docs_get_page", &available).unwrap();
    let value = serde_json::to_value(resolution).unwrap();
    let object = value.as_object().unwrap();

    assert_eq!(object.len(), 6);
    assert!(object.contains_key("instance_id"));
    assert!(object.contains_key("service_name"));
    assert!(object.contains_key("scope"));
    assert!(object.contains_key("tool_name"));
    assert!(object.contains_key("resolution_method"));
    assert!(object.contains_key("original_input"));
}
