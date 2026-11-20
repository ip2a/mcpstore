use super::*;
use crate::config::{ScopeDeclarations, ScopeDescriptor};
use crate::identity::{ScopeRef, ServiceInstanceKey};
use serde_json::Map;

#[tokio::test]
async fn test_registry_lifecycle() {
    let reg = ServiceRegistry::new();
    let definition = ServiceDefinition {
        service_name: "svc1".to_string(),
        base_config: Map::new(),
        scopes: ScopeDeclarations::store_only(),
        lifecycle: None,
        handshake_mode: None,
        base_revision: 1,
        metadata: Map::new(),
        added_time: 1234567890,
    };
    let instance_id = ServiceInstanceKey::new("svc1", ScopeRef::Store).instance_id();
    let instance = ServiceInstance {
        instance_id,
        service_name: "svc1".to_string(),
        scope: ScopeRef::Store,
        transport: "stdio".to_string(),
        url: None,
        command: Some("python tool.py".to_string()),
        tools: vec![ToolInfo {
            name: "tool1".to_string(),
            title: None,
            description: "desc".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: None,
            annotations: None,
            icons: None,
            meta: None,
        }],
        effective_config: Map::new(),
        config_revision: ConfigRevision {
            base_revision: 1,
            scope_revision: 1,
        },
        applied_config_revision: Some(ConfigRevision {
            base_revision: 1,
            scope_revision: 1,
        }),
        added_time: 1234567890,
    };

    reg.register_definition(definition.clone()).await;
    reg.register_instance(instance.clone()).await;
    assert_eq!(reg.find_definition("svc1").await, Some(definition.clone()));
    assert_eq!(reg.find_instance(instance_id).await, Some(instance.clone()));
    assert_eq!(
        reg.find_instance_by_key("svc1", &ScopeRef::Store).await,
        Some(instance.clone())
    );

    let tool = reg.find_tool(instance_id, "tool1").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name, "tool1");

    reg.unregister_definition("svc1").await;
    assert_eq!(reg.find_definition("svc1").await, None);
    assert_eq!(reg.find_instance(instance_id).await, None);
    assert_eq!(reg.find_tool(instance_id, "tool1").await, None);
}

#[tokio::test]
async fn same_service_name_has_isolated_scope_instances() {
    let reg = ServiceRegistry::new();
    let mut scopes = ScopeDeclarations::store_only();
    scopes
        .agents
        .insert("agent1".to_string(), ScopeDescriptor::default());
    let definition = ServiceDefinition {
        service_name: "svc".to_string(),
        base_config: Map::new(),
        scopes,
        lifecycle: None,
        handshake_mode: None,
        base_revision: 1,
        metadata: Map::new(),
        added_time: 1,
    };
    reg.register_definition(definition).await;

    for scope in [
        ScopeRef::Store,
        ScopeRef::Agent {
            agent_id: "agent1".to_string(),
        },
    ] {
        let instance_id = ServiceInstanceKey::new("svc", scope.clone()).instance_id();
        reg.register_instance(ServiceInstance {
            instance_id,
            service_name: "svc".to_string(),
            scope,
            transport: "stdio".to_string(),
            url: None,
            command: Some("demo".to_string()),
            tools: Vec::new(),
            effective_config: Map::new(),
            config_revision: ConfigRevision {
                base_revision: 1,
                scope_revision: 1,
            },
            applied_config_revision: None,
            added_time: 1,
        })
        .await;
    }

    let store_id = reg.instance_id("svc", &ScopeRef::Store).await.unwrap();
    let agent_id = reg
        .instance_id(
            "svc",
            &ScopeRef::Agent {
                agent_id: "agent1".to_string(),
            },
        )
        .await
        .unwrap();
    assert_ne!(store_id, agent_id);

    assert_eq!(
        reg.find_instance(store_id).await.unwrap().scope,
        ScopeRef::Store
    );
    assert_eq!(
        reg.find_instance(agent_id).await.unwrap().scope,
        ScopeRef::Agent {
            agent_id: "agent1".to_string(),
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn swap_all_is_atomic_for_concurrent_readers() {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn store_instance(service_name: &str) -> ServiceInstance {
        let instance_id = ServiceInstanceKey::new(service_name, ScopeRef::Store).instance_id();
        ServiceInstance {
            instance_id,
            service_name: service_name.to_string(),
            scope: ScopeRef::Store,
            transport: "stdio".to_string(),
            url: None,
            command: Some("demo".to_string()),
            tools: Vec::new(),
            effective_config: Map::new(),
            config_revision: ConfigRevision {
                base_revision: 1,
                scope_revision: 1,
            },
            applied_config_revision: None,
            added_time: 1,
        }
    }

    fn generation(generation: u32) -> (HashMap<String, ServiceDefinition>, Vec<ServiceInstance>) {
        let mut definitions = HashMap::new();
        let mut instances = Vec::new();
        for suffix in 0..4 {
            let name = format!("svc-gen{generation}-{suffix}");
            definitions.insert(
                name.clone(),
                ServiceDefinition {
                    service_name: name.clone(),
                    base_config: Map::new(),
                    scopes: ScopeDeclarations::store_only(),
                    lifecycle: None,
                    handshake_mode: None,
                    base_revision: 1,
                    metadata: Map::new(),
                    added_time: 1,
                },
            );
            instances.push(store_instance(&name));
        }
        (definitions, instances)
    }

    let reg = ServiceRegistry::new();
    let (definitions, instances) = generation(0);
    reg.swap_all(definitions, instances).await;

    let stop = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..4 {
        let reg = reg.clone();
        let stop = Arc::clone(&stop);
        readers.push(tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                let instances = reg.list_instances().await;
                // A swap replaces whole generations atomically: each single read
                // observes the previous or the next complete view — every snapshot
                // is non-empty and all its instances come from one generation.
                assert!(!instances.is_empty());
                let mut generations = instances.iter().map(|instance| {
                    instance.service_name.split('-').nth(1).unwrap().to_string()
                });
                let view = generations.next().unwrap();
                assert!(generations.all(|gen| gen == view), "torn view: {instances:?}");
                for instance in &instances {
                    let expected_id = ServiceInstanceKey::new(
                        &instance.service_name,
                        instance.scope.clone(),
                    )
                    .instance_id();
                    assert_eq!(expected_id, instance.instance_id);
                }
            }
        }));
    }

    for current in 1..200u32 {
        let (definitions, instances) = generation(current);
        reg.swap_all(definitions, instances).await;
        tokio::task::yield_now().await;
    }
    stop.store(true, Ordering::Relaxed);
    for reader in readers {
        reader.await.unwrap();
    }
}
