//! Placement × scope interaction tests.
//!
//! Documents current merge semantics as regression anchors for the design review:
//! - scope config applies at registration time (`instance.effective_config`)
//! - placement diff merges over `definition.base_config` on the data panel, ignoring scope config
//! - placement keys are independent of declared agent scopes (orthogonal axes)

use serde_json::{json, Value};

use crate::config::{McpStoreExtension, ScopeDeclarations, ScopeDescriptor, ServerConfig};
use crate::identity::{InstanceId, ScopeRef, ServiceInstanceKey};
use crate::store::{JsonStoreConfig, MCPStore, PanelRole, StoreOptions};

fn agent(agent_id: &str) -> ScopeRef {
    ScopeRef::Agent {
        agent_id: agent_id.to_string(),
    }
}

fn instance_of(service: &str, scope: ScopeRef) -> InstanceId {
    ServiceInstanceKey::new(service.to_string(), scope).instance_id()
}

fn control_panel(ns: String) -> std::sync::Arc<MCPStore> {
    MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(ns),
        panel: PanelRole::ControlPanel,
        ..Default::default()
    })
    .unwrap()
}

fn data_panel(ns: String, panel_id: &str) -> std::sync::Arc<MCPStore> {
    MCPStore::setup_with_options(StoreOptions {
        config_path: None,
        store: Some(JsonStoreConfig::shared_memory()),
        namespace: Some(ns),
        panel: PanelRole::DataPanel {
            panel_id: panel_id.to_string(),
        },
        ..Default::default()
    })
    .unwrap()
}

fn config_with(
    base_args: Vec<String>,
    scope_args: Option<Vec<String>>,
    placement: Vec<(&str, Value)>,
) -> ServerConfig {
    let mut config = ServerConfig {
        command: Some("base-cmd".to_string()),
        args: base_args,
        transport: Some("stdio".to_string()),
        ..Default::default()
    };
    config.mcpstore = Some(McpStoreExtension {
        scopes: ScopeDeclarations {
            store: Some(ScopeDescriptor::default()),
            agents: [(
                "dev01".to_string(),
                ScopeDescriptor {
                    config: scope_args
                        .map(|args| json!({ "args": args }).as_object().unwrap().clone())
                        .unwrap_or_default(),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
        },
        placement: placement
            .into_iter()
            .map(|(panel, diff)| (panel.to_string(), diff))
            .collect(),
        ..Default::default()
    });
    config
}

fn args_of(config: &serde_json::Map<String, Value>) -> Vec<String> {
    config["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn placement_merge_ignores_scope_config() {
    // base.args=[base] · scope(dev01).config.args=[scope] · placement[edge-01].args=[placement]
    let ns = format!("place-precedence-{}", uuid::Uuid::new_v4());
    let store = control_panel(ns);
    store
        .add_service(
            "svc",
            config_with(
                vec!["base".into()],
                Some(vec!["scope".into()]),
                vec![("edge-01", json!({ "args": ["placement"] }))],
            ),
        )
        .await
        .unwrap();

    // Registration time: the agent instance carries the scope override.
    let instance = store
        .find_instance(instance_of("svc", agent("dev01")))
        .await
        .unwrap();
    assert_eq!(args_of(&instance.effective_config), ["scope"]);

    // Execution time on the data panel: merge is base + placement diff, scope config is not an input.
    let definition = store.find_definition("svc").await.unwrap();
    let diff = definition.placement["edge-01"].as_object().unwrap();
    let merged = crate::config::merge_config(&definition.base_config, diff);
    assert_eq!(args_of(&merged), ["placement"]);
    assert_eq!(merged["command"].as_str().unwrap(), "base-cmd");
}

#[tokio::test]
async fn placement_panel_without_matching_agent_scope_is_accepted() {
    // placement declares "edge-99" but no agent scope for it — the two axes are orthogonal.
    let ns = format!("place-orphan-{}", uuid::Uuid::new_v4());
    let store = control_panel(ns);
    store
        .add_service(
            "orphan",
            config_with(
                vec!["base".into()],
                None,
                vec![("edge-99", json!({ "env": { "KEY": "val" } }))],
            ),
        )
        .await
        .unwrap();

    let definition = store.find_definition("orphan").await.unwrap();
    assert_eq!(definition.placement.len(), 1);
    assert!(definition.placement.contains_key("edge-99"));
    // Only the store scope exists; placement did not implicitly declare any scope.
    assert!(store
        .find_instance(instance_of("orphan", ScopeRef::Store))
        .await
        .is_some());
}

#[tokio::test]
async fn scope_without_placement_stays_visible_to_data_panel() {
    // No placement at all → data panel sees the instance via kv, and routing falls to remote RPC.
    let ns = format!("place-rpc-fallback-{}", uuid::Uuid::new_v4());
    let _control = control_panel(ns.clone());
    _control
        .add_service("no-place", config_with(vec!["base".into()], None, vec![]))
        .await
        .unwrap();

    let data = data_panel(ns, "edge-01");
    let id = instance_of("no-place", agent("dev01"));
    assert!(data.find_instance(id).await.is_some());

    let definition = data.find_definition("no-place").await.unwrap();
    assert!(!definition.placement.contains_key("edge-01"));
}

#[tokio::test]
async fn empty_placement_key_currently_accepted_without_validation() {
    // Documents the gap: add_service performs no placement key validation today.
    let ns = format!("place-empty-key-{}", uuid::Uuid::new_v4());
    let store = control_panel(ns);
    let result = store
        .add_service(
            "bad-place",
            config_with(vec!["base".into()], None, vec![("", json!({}))]),
        )
        .await;

    match result {
        Ok(()) => {
            let definition = store.find_definition("bad-place").await.unwrap();
            assert!(definition.placement.contains_key(""));
        }
        Err(error) => {
            let message = error.to_string();
            assert!(
                message.contains("placement") || message.contains("panel"),
                "rejection should mention placement, got: {message}"
            );
        }
    }
}
