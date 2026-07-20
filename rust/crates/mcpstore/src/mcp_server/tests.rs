#[cfg(test)]
mod tests {
    use super::super::catalog::*;
    use super::super::tools::*;
    use super::super::transport::normalize_http_path;
    use super::super::*;
    use crate::{events::types::EventKind, CacheStorage, ServiceInstanceKey};
    use rmcp::{
        service::{NotificationContext, RoleClient},
        ClientHandler, ServiceExt,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Default)]
    struct NotificationClient {
        tool_list_changes: Arc<AtomicUsize>,
    }

    impl ClientHandler for NotificationClient {
        async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
            self.tool_list_changes.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn launch_descriptor_matches_the_runtime_cli_contract() {
        let descriptor = McpServerOptions {
            scope: ScopeRef::Agent {
                agent_id: "coding".into(),
            },
            transport: McpServerTransport::StreamableHttp,
            host: "127.0.0.1".into(),
            port: 19000,
            path: "/mcp".into(),
            ..Default::default()
        }
        .launch_descriptor("mcpstore");
        assert_eq!(descriptor.command, None);
        assert_eq!(
            descriptor.url.as_deref(),
            Some("http://127.0.0.1:19000/mcp")
        );
        assert_eq!(
            descriptor.args,
            [
                "mcp-server",
                "--transport",
                "streamable-http",
                "--scope",
                "agent",
                "--agent",
                "coding"
            ]
        );
    }

    #[tokio::test]
    async fn aggregate_server_forwards_scoped_tool_list_changes() {
        let store = MCPStore::setup_with_options(StoreOptions {
            backend: Some(CacheStorage::Memory),
            ..StoreOptions::default()
        })
        .unwrap();
        let instance_id = ServiceInstanceKey::new("aggregate", ScopeRef::Store).instance_id();
        let other_instance_id = ServiceInstanceKey::new("other", ScopeRef::Store).instance_id();
        let server = McpStoreServer {
            store: Arc::clone(&store),
            scope: ScopeRef::Store,
            instance_id: Some(instance_id),
            session_key: None,
            scope_label: "store".to_string(),
            bindings: Arc::new(HashMap::new()),
            session_state_tools: Arc::new(HashMap::new()),
            tool_transform_tools: Arc::new(HashMap::new()),
            openapi_tools: Arc::new(HashMap::new()),
            service_tools: Arc::new(HashMap::new()),
            cache_tools: Arc::new(HashMap::new()),
            event_tools: Arc::new(HashMap::new()),
            tools: Arc::new(Vec::new()),
        };
        let client_handler = NotificationClient::default();
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server_start =
            tokio::spawn(async move { server.serve(server_transport).await.unwrap() });
        let client = client_handler
            .clone()
            .serve(client_transport)
            .await
            .unwrap();
        let server = server_start.await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                store
                    .event_bus
                    .publish(
                        Event::new(
                            EventKind::McpToolsChanged.as_str(),
                            serde_json::json!({"instanceId": instance_id}),
                        ),
                        true,
                    )
                    .await;
                if client_handler.tool_list_changes.load(Ordering::SeqCst) == 1 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        store
            .event_bus
            .publish(
                Event::new(
                    EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({"instanceId": other_instance_id}),
                ),
                true,
            )
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert_eq!(client_handler.tool_list_changes.load(Ordering::SeqCst), 1);

        client.cancel().await.unwrap();
        server.cancel().await.unwrap();
        store
            .event_bus
            .publish(
                Event::new(
                    EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({"instanceId": instance_id}),
                ),
                true,
            )
            .await;
    }

    #[test]
    fn duplicate_prompt_names_are_stably_projected_and_routed() {
        let first = ServiceInstanceKey::new("first service", ScopeRef::Store).instance_id();
        let second = ServiceInstanceKey::new("second", ScopeRef::Store).instance_id();
        let prompts = vec![
            serde_json::json!({
                "name": "review",
                "service_name": "first service",
                "instance_id": first,
            }),
            serde_json::json!({
                "name": "review",
                "service_name": "second",
                "instance_id": second,
            }),
        ];

        let projected = project_prompt_names(prompts.clone()).unwrap();
        let first_name = projected[0]["name"].as_str().unwrap();
        assert_eq!(first_name, format!("first_service__{first}__review"));
        assert_eq!(
            resolve_projected_prompt(&prompts, first_name).unwrap(),
            (first, "review".to_string())
        );
    }

    #[test]
    fn resource_and_template_uris_are_stably_projected_and_routed() {
        let instance_id = ServiceInstanceKey::new("docs service", ScopeRef::Store).instance_id();
        let resource = serde_json::json!({
            "uri": "fixture://docs/readme",
            "service_name": "docs service",
            "instance_id": instance_id,
        });
        let template = serde_json::json!({
            "uri_template": "fixture://docs/{name}",
            "service_name": "docs service",
            "instance_id": instance_id,
        });

        let projected_resource =
            project_catalog_uris(vec![resource.clone()], "uri", false).unwrap();
        let projected_resource_uri = projected_resource[0]["uri"].as_str().unwrap();
        assert_eq!(
            resolve_projected_catalog_uri(&[resource], "uri", false, projected_resource_uri)
                .unwrap(),
            (instance_id, "fixture://docs/readme".to_string())
        );

        let projected_template =
            project_catalog_uris(vec![template.clone()], "uri_template", true).unwrap();
        let projected_template_uri = projected_template[0]["uri_template"].as_str().unwrap();
        assert!(projected_template_uri.contains("/template/"));
        assert_eq!(
            resolve_projected_catalog_uri(
                &[template],
                "uri_template",
                true,
                projected_template_uri,
            )
            .unwrap(),
            (instance_id, "fixture://docs/{name}".to_string())
        );
    }

    #[test]
    fn structured_scope_argument_has_no_name_fallback() {
        let store = Map::from_iter([("scope".to_string(), serde_json::json!({"type": "store"}))]);
        assert_eq!(
            required_scope_argument(&store).expect("store scope"),
            ScopeRef::Store
        );

        let agent = Map::from_iter([(
            "scope".to_string(),
            serde_json::json!({"type": "agent", "agent_id": "agent-a"}),
        )]);
        assert_eq!(
            required_scope_argument(&agent).expect("agent scope"),
            ScopeRef::Agent {
                agent_id: "agent-a".to_string()
            }
        );

        let legacy = Map::from_iter([("scope".to_string(), Value::String("agent-a".to_string()))]);
        assert!(required_scope_argument(&legacy).is_err());
    }

    #[test]
    fn runtime_schema_requires_instance_id() {
        for schema in [instance_id_schema(), service_wait_schema()] {
            assert_eq!(
                schema.get("required"),
                Some(&serde_json::json!(["instance_id"]))
            );
            assert!(schema["properties"].get("name").is_none());
            assert!(schema["properties"].get("service_name").is_none());
        }

        let service_tools = build_service_tools();
        let check_tool = service_tools
            .get(SERVICE_CHECK_TOOL)
            .expect("instance check tool");
        assert_eq!(
            check_tool.input_schema.get("required"),
            Some(&serde_json::json!(["instance_id"]))
        );
    }

    #[test]
    fn config_schema_requires_service_name_and_scope() {
        for schema in [
            service_scope_schema(),
            service_add_schema(),
            service_scope_descriptor_schema(),
        ] {
            let required = schema["required"]
                .as_array()
                .expect("required fields must be an array");
            assert!(required.contains(&Value::String("service_name".to_string())));
            assert!(required.contains(&Value::String("scope".to_string())));
            assert!(schema["properties"].get("instance_id").is_none());
        }
    }

    #[test]
    fn http_path_normalization_is_stable() {
        assert_eq!(normalize_http_path(""), "/mcp");
        assert_eq!(normalize_http_path("custom/"), "/custom");
        assert_eq!(normalize_http_path("/custom/"), "/custom");
    }
}
