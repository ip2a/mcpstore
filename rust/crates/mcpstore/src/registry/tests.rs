use super::*;

#[tokio::test]
async fn test_registry_lifecycle() {
    let reg = ServiceRegistry::new();

    let entry = ServiceEntry {
        name: "svc1".to_string(),
        original_name: "svc1".to_string(),
        agent_id: "global_agent_store".to_string(),
        transport: "stdio".to_string(),
        url: None,
        command: Some("python tool.py".to_string()),
        status: ConnectionStatus::Connected,
        tools: vec![ToolInfo {
            name: "tool1".to_string(),
            description: "desc".to_string(),
            schema: serde_json::json!({}),
        }],
        config: serde_json::json!({}),
        added_time: 1234567890,
    };

    reg.register(entry.clone()).await;
    assert_eq!(reg.find_service("svc1").await, Some(entry.clone()));

    let tool = reg.find_tool("svc1_tool1").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name, "tool1");

    reg.unregister("svc1").await;
    assert_eq!(reg.find_service("svc1").await, None);
    assert_eq!(reg.find_tool("svc1_tool1").await, None);
}
