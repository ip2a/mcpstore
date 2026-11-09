#[cfg(test)]
mod tests {
    use super::super::catalog::*;
    use super::super::tools::*;
    use super::super::transport::normalize_http_path;
    use super::super::*;
    use mcpstore::{events::types::EventKind, JsonStoreConfig, ServiceInstanceKey};
    use rmcp::{
        model::{ProtocolVersion, ServerNotification, SubscriptionFilter},
        service::ClientLifecycleMode,
        ClientHandler, ServiceExt,
    };

    #[derive(Clone, Default)]
    struct NotificationClient;

    impl ClientHandler for NotificationClient {}

    #[test]
    fn launch_descriptor_reports_the_http_url() {
        let descriptor = GatewayOptions {
            scope: ScopeRef::Agent {
                agent_id: "coding".into(),
            },
            host: "127.0.0.1".into(),
            port: 19000,
            path: "/mcp".into(),
            ..Default::default()
        }
        .launch_descriptor();
        assert_eq!(descriptor.url, "http://127.0.0.1:19000/mcp");
    }

    #[tokio::test]
    async fn gateway_server_forwards_scoped_tool_list_changes() {
        let store = MCPStore::setup_with_options(StoreOptions {
            store: Some(JsonStoreConfig::memory()),
            ..StoreOptions::default()
        })
        .unwrap();
        let instance_id = ServiceInstanceKey::new("aggregate", ScopeRef::Store).instance_id();
        let other_instance_id = ServiceInstanceKey::new("other", ScopeRef::Store).instance_id();
        let server = GatewayServer {
            store: Arc::clone(&store),
            scope: ScopeRef::Store,
            instance_id: Some(instance_id),
            session_key: None,
            scope_label: "store".to_string(),
            bindings: Arc::new(HashMap::new()),
            session_state_tools: Arc::new(HashMap::new()),
            tool_override_tools: Arc::new(HashMap::new()),
            prompt_override_tools: Arc::new(HashMap::new()),
            resource_override_tools: Arc::new(HashMap::new()),
            openapi_tools: Arc::new(HashMap::new()),
            service_tools: Arc::new(HashMap::new()),
            cache_tools: Arc::new(HashMap::new()),
            search_tools: Arc::new(HashMap::new()),
            tools: Arc::new(Vec::new()),
            caller: None,
            call_log: GatewayCallLog::new(),
            down: Arc::new(HashMap::new()),
            projection: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let client_handler = NotificationClient::default();
        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server_start =
            tokio::spawn(async move { server.serve(server_transport).await.unwrap() });
        let client = rmcp::service::serve_client_with_lifecycle(
            client_handler,
            client_transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .unwrap();

        let mut subscription = client
            .listen(SubscriptionFilter::builder().tools_list_changed().build())
            .await
            .unwrap();
        let server = server_start.await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store
            .event_bus()
            .publish(
                Event::new(
                    EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({"instanceId": instance_id}),
                ),
                true,
            )
            .await;
        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(1), subscription.next())
                .await
                .unwrap()
                .unwrap(),
            Some(ServerNotification::ToolListChangedNotification(_))
        ));

        store
            .event_bus()
            .publish(
                Event::new(
                    EventKind::McpToolsChanged.as_str(),
                    serde_json::json!({"instanceId": other_instance_id}),
                ),
                true,
            )
            .await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), subscription.next())
                .await
                .is_err()
        );

        subscription.cancel().await.unwrap();
        client.cancel().await.unwrap();
        server.cancel().await.unwrap();
        store
            .event_bus()
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
        assert_eq!(first_name, format!("first-service__{first}__review"));
        assert_eq!(
            resolve_projected_prompt(&prompts, first_name).unwrap(),
            (first, "review".to_string(), "first service".to_string())
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
            (instance_id, "fixture://docs/readme".to_string(), "docs service".to_string())
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
            (instance_id, "fixture://docs/{name}".to_string(), "docs service".to_string())
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
    fn component_override_schemas_match_component_fields() {
        let tool = tool_override_schema(&[]);
        assert!(tool["properties"].get("arguments").is_some());
        assert!(tool["properties"].get("safety_policy").is_some());
        assert!(tool["properties"].get("meta").is_some());

        let prompt = build_prompt_override_tools();
        let prompt_schema = &prompt[PROMPT_OVERRIDE_SET_TOOL].input_schema;
        assert!(prompt_schema["properties"].get("mime_type").is_none());
        assert!(prompt_schema["properties"].get("arguments").is_none());

        for tools in [
            build_resource_override_tools(),
            build_resource_template_override_tools(),
        ] {
            let schema = &tools.values().next().expect("override tool").input_schema;
            assert!(schema["properties"].get("mime_type").is_some());
            assert_eq!(schema["additionalProperties"], Value::Bool(false));
        }
    }

    #[test]
    fn http_path_normalization_is_stable() {
        assert_eq!(normalize_http_path(""), "/mcp");
        assert_eq!(normalize_http_path("custom/"), "/custom");
        assert_eq!(normalize_http_path("/custom/"), "/custom");
    }

    #[test]
    fn service_overrides_filter_and_rename_catalog_payloads() {
        use mcpstore::{ComponentOverrideCommon, ServiceOverrideRule};
        let first = ServiceInstanceKey::new("github", ScopeRef::Store).instance_id();
        let second = ServiceInstanceKey::new("docs", ScopeRef::Store).instance_id();
        let disabled = ServiceInstanceKey::new("secret", ScopeRef::Store).instance_id();

        let rule = |instance_id, name: &str, display: Option<&str>, enabled: Option<bool>| {
            ServiceOverrideRule {
                instance_id,
                service_name: name.to_string(),
                scope: ScopeRef::Store,
                common: ComponentOverrideCommon {
                    display_name: display.map(str::to_string),
                    enabled,
                    ..Default::default()
                },
                updated_at: 0,
                version: 1,
            }
        };
        let overrides = HashMap::from([
            (first, rule(first, "github", Some("Git 工具"), None)),
            (disabled, rule(disabled, "secret", None, Some(false))),
        ]);

        let payloads = vec![
            serde_json::json!({"name": "create_issue", "instance_id": first, "service_name": "github"}),
            serde_json::json!({"name": "read", "instance_id": second, "service_name": "docs"}),
            serde_json::json!({"name": "leak", "instance_id": disabled, "service_name": "secret"}),
        ];
        let projected = apply_service_overrides(payloads, &overrides).unwrap();
        assert_eq!(projected.len(), 2);
        assert_eq!(
            projected[0]["service_name"].as_str(),
            Some("Git 工具"),
            "overridden service exposes its display name"
        );
        assert_eq!(
            projected[0]["real_service_name"].as_str(),
            Some("github"),
            "real name kept aside for observability joins"
        );
        assert_eq!(
            projected[1]["service_name"].as_str(),
            Some("docs"),
            "unoverridden service passes through unchanged"
        );
        assert!(
            !projected.iter().any(|payload| payload["name"] == "leak"),
            "disabled service must vanish from the catalog"
        );
    }

    #[test]
    fn exposed_tool_name_collision_suffix() {
        let mut taken = HashMap::new();
        assert_eq!(exposed_tool_name("srv", "echo", &mut taken), "srv__echo");
        assert_eq!(exposed_tool_name("srv", "echo", &mut taken), "srv__echo_2");
        assert_eq!(exposed_tool_name("srv", "echo", &mut taken), "srv__echo_3");
        assert_eq!(exposed_tool_name("other", "echo", &mut taken), "other__echo");
    }

    #[test]
    fn truncate_tool_name_respects_utf8_boundaries() {
        assert_eq!(truncate_tool_name("abc", 10), "abc");
        assert_eq!(truncate_tool_name("abcdef", 3), "abc");
        // 4-byte emoji just past the cut: must back off to the previous boundary, not panic/mangle.
        let name = "ab🙂cd";
        let truncated = truncate_tool_name(name, 3);
        assert!(name.is_char_boundary(truncated.len()));
        assert_eq!(truncated, "ab");
    }

    #[test]
    fn service_namespace_replaces_unsafe_chars_with_hyphen() {
        assert_eq!(service_namespace("first service"), "first-service");
        assert_eq!(service_namespace("a_b-c9"), "a_b-c9");
    }

    /// Inclusive gateway contract: bare legacy callers (no headers, no _meta),
    /// callers naming the negotiated 2025-11-25 per request, and initialize
    /// handshakes all succeed against the same stateless router. Regression
    /// for the handshake/per-request version contradiction.
    #[tokio::test]
    async fn gateway_serves_bare_and_negotiated_version_requests() {
        let store = MCPStore::setup_with_options(StoreOptions {
            store: Some(JsonStoreConfig::memory()),
            ..StoreOptions::default()
        })
        .unwrap();
        let server = GatewayServer {
            store: Arc::clone(&store),
            scope: ScopeRef::Store,
            instance_id: None,
            session_key: None,
            scope_label: "store".to_string(),
            bindings: Arc::new(HashMap::new()),
            session_state_tools: Arc::new(HashMap::new()),
            tool_override_tools: Arc::new(HashMap::new()),
            prompt_override_tools: Arc::new(HashMap::new()),
            resource_override_tools: Arc::new(HashMap::new()),
            openapi_tools: Arc::new(HashMap::new()),
            service_tools: Arc::new(HashMap::new()),
            cache_tools: Arc::new(HashMap::new()),
            search_tools: Arc::new(HashMap::new()),
            tools: Arc::new(Vec::new()),
            caller: None,
            call_log: GatewayCallLog::new(),
            down: Arc::new(HashMap::new()),
            projection: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router =
            super::super::transport::streamable_http_router(server, "/mcp");
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::new();
        let url = format!("http://{addr}/mcp");

        // Initialize at a legacy revision, with curl's default Accept: the
        // handshake must echo the requested version back.
        let initialize = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "clientInfo": { "name": "curl", "version": "0" },
                "capabilities": {}
            }
        });
        let response = client
            .post(&url)
            .header("Accept", "*/*")
            .json(&initialize)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["result"]["protocolVersion"], "2025-11-25");

        // Bare request: no MCP-Protocol-Version header, no _meta, curl Accept.
        let bare = serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
        let response = client
            .post(&url)
            .header("Accept", "*/*")
            .json(&bare)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body["result"]["tools"].is_array(), "bare tools/list failed: {body}");

        // Per-request inline version naming the negotiated revision (P0-1).
        let versioned = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list",
            "params": {
                "_meta": { "io.modelcontextprotocol/protocolVersion": "2025-11-25" }
            }
        });
        let response = client.post(&url).json(&versioned).send().await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(
            body["result"]["tools"].is_array(),
            "versioned tools/list failed: {body}"
        );
    }

    #[test]
    fn call_records_carry_caller_and_service_name() {
        let instance_id = ServiceInstanceKey::new("demo", ScopeRef::Store).instance_id();
        let log = GatewayCallLog::new();
        log.record(
            Some("claude"),
            GatewayCallKind::Tool,
            "demo__echo",
            instance_id,
            "demo",
            std::time::Duration::from_millis(3),
            false,
        );

        let records = log.snapshot_after(0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].caller.as_deref(), Some("claude"));
        assert_eq!(records[0].service_name, "demo");
        assert_eq!(records[0].name, "demo__echo");
        assert_eq!(records[0].kind, GatewayCallKind::Tool);
        assert!(!records[0].success);
        assert_eq!(records[0].duration_ms, 3);
    }

    #[test]
    fn projection_classify_isolates_defects_without_touching_healthy_tools() {
        use super::super::transport::{classify_tool_payloads, DownService};
        use std::collections::HashMap as StdHashMap;

        let healthy_instance = ServiceInstanceKey::new("svc", ScopeRef::Store).instance_id();
        let down_instance = ServiceInstanceKey::new("dead", ScopeRef::Store).instance_id();
        let tool = |name: &str, instance_id: &str, service_name: &str| {
            serde_json::json!({
                "name": name,
                "tool_name": name,
                "instance_id": instance_id,
                "service_name": service_name,
                "input_schema": {"type": "object", "properties": {}},
                "description": format!("{name} tool")
            })
        };
        let healthy_id = healthy_instance.to_string();
        let down_id = down_instance.to_string();
        let payloads = vec![
            tool("healthy", &healthy_id, "svc"),
            // Missing name entirely.
            serde_json::json!({
                "tool_name": "broken",
                "instance_id": healthy_id,
                "service_name": "svc",
                "input_schema": {"type": "object"}
            }),
            // Missing tool_name.
            {
                let mut value = tool("no-canonical", &healthy_id, "svc");
                value["tool_name"] = serde_json::Value::Null;
                value
            },
            // Missing schema.
            {
                let mut value = tool("no-schema", &healthy_id, "svc");
                value["input_schema"] = serde_json::Value::Null;
                value
            },
            // Belongs to a down service.
            tool("dead-tool", &down_id, "dead"),
        ];

        let mut down = StdHashMap::new();
        down.insert(
            down_instance,
            DownService {
                service_name: "dead".to_string(),
                reason: "spawn failed".to_string(),
            },
        );

        let (parsed, issues) = classify_tool_payloads(&payloads, &down);

        let exposed: Vec<&str> = parsed
            .iter()
            .map(|(_, _, name, _, _, _)| name.as_str())
            .collect();
        assert_eq!(exposed, ["healthy", "no-schema"]);
        let degraded: Vec<&str> = parsed
            .iter()
            .filter(|(_, _, _, _, _, schema)| schema.is_none())
            .map(|(_, _, name, _, _, _)| name.as_str())
            .collect();
        assert_eq!(degraded, ["no-schema"]);

        assert!(matches!(
            issues.first(),
            Some(ProjectionIssue::Down { service, .. }) if service == "dead"
        ));
        assert!(issues.iter().any(|issue| matches!(
            issue,
            ProjectionIssue::SkippedTool { tool: None, .. }
        )));
        assert!(issues.iter().any(|issue| matches!(
            issue,
            ProjectionIssue::SkippedTool { tool: Some(name), .. } if name == "no-canonical"
        )));
        assert!(issues.iter().any(|issue| matches!(
            issue,
            ProjectionIssue::DegradedTool { service, tool } if service == "svc" && tool == "no-schema"
        )));
    }

    #[tokio::test]
    async fn unconnectable_service_downs_its_tools_but_not_the_mount() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let store_path = std::env::temp_dir().join(format!("gateway-down-{nanos}.json"));
        std::fs::write(&store_path, b"{}").unwrap();
        let store = Arc::new(
            MCPStore::setup_with_options(mcpstore::StoreOptions {
                config_path: Some(store_path.to_string_lossy().into_owned()),
                store: Some(JsonStoreConfig::memory()),
                namespace: Some(format!("gateway-down-{nanos}")),
                ..mcpstore::StoreOptions::default()
            })
            .unwrap(),
        );
        store
            .add_service(
                "ghost",
                mcpstore::ServerConfig {
                    command: Some("mcpstore-no-such-binary-xyz".to_string()),
                    ..mcpstore::ServerConfig::default()
                },
            )
            .await
            .unwrap();

        let options = GatewayOptions {
            scope: ScopeRef::Store,
            ..Default::default()
        };
        let server = GatewayServer::from_store(
            Arc::clone(&store),
            &options,
            None,
            GatewayCallLog::new(),
        )
        .await
        .expect("mount must build even when a service cannot connect");

        let projection = server.projection.lock().unwrap();
        assert!(projection.iter().any(|issue| matches!(
            issue,
            ProjectionIssue::Down { service, .. } if service == "ghost"
        )));
        let _ = std::fs::remove_file(store_path);
    }

    #[test]
    fn gateway_errors_carry_agent_actionable_guidance() {
        use mcpstore::error::FailureCode;

        let error = mcpstore::Error::new(FailureCode::ServiceNotFound, "missing");
        let mapped = map_store_error(mcpstore::Error::new(FailureCode::ServiceNotFound, "missing"));
        let data = mapped
            .data
            .as_ref()
            .and_then(Value::as_object)
            .expect("structured data payload")
            .clone();
        assert_eq!(data["code"], "service_not_found");
        assert!(data["hint"]
            .as_str()
            .unwrap()
            .contains("call list_services"));
        assert!(data.get("retryable").is_some());

        let result = super::super::handler::llm_error_result(&error);
        let CallToolResponse::Complete(result) = result else {
            panic!("expected complete response");
        };
        assert_eq!(result.is_error, Some(true));
        let text = &result.content[0].as_text().expect("text content").text;
        let payload: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(payload["code"], "service_not_found");
        assert!(payload["hint"].as_str().unwrap().contains("list_services"));

        let single = unknown_tool_error("x", true);
        let multi = unknown_tool_error("x", false);
        let single_data = single.data.as_ref().and_then(Value::as_object).unwrap();
        let multi_data = multi.data.as_ref().and_then(Value::as_object).unwrap();
        assert_ne!(single_data, multi_data);
        assert!(single_data["hint"].as_str().unwrap().contains("tools/list"));
        assert!(multi_data["hint"].as_str().unwrap().contains("down"));
    }
}
