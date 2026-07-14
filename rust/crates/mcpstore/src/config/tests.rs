use serde_json::{json, Value};

use super::{examples::default_server_config, *};
use crate::identity::ScopeRef;

#[test]
fn test_load_save_roundtrip() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mcp.json");

    let mgr = ConfigManager::with_path(&path);
    let mut config = McpConfig::default();
    config.mcp_servers.insert(
        "test".to_string(),
        ServerConfig {
            url: Some("http://localhost:8080/mcp".to_string()),
            transport: Some("streamable-http".to_string()),
            ..default_server_config()
        },
    );

    mgr.save(&config).unwrap();
    let loaded = mgr.load().unwrap();
    assert_eq!(loaded.mcp_servers.len(), 1);
    assert!(loaded.mcp_servers.contains_key("test"));
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("cache"));
    assert_eq!(
        loaded.mcp_servers["test"].scopes(),
        ScopeDeclarations::store_only()
    );
    assert!(raw.contains("\"scopes\""));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn third_party_service_without_extension_has_store_scope() {
    let config: McpConfig = serde_json::from_value(json!({
        "mcpServers": {
            "demo": {
                "command": "demo",
                "futureTransportOption": {
                    "mode": "strict"
                }
            }
        }
    }))
    .unwrap();

    let service = &config.mcp_servers["demo"];
    assert_eq!(service.scopes(), ScopeDeclarations::store_only());
    assert_eq!(
        service.extra["futureTransportOption"],
        json!({ "mode": "strict" })
    );
}

#[test]
fn native_extension_requires_scopes() {
    let error = serde_json::from_value::<McpConfig>(json!({
        "mcpServers": {
            "demo": {
                "command": "demo",
                "_mcpstore": {
                    "lifecycle": {
                        "startup_policy": "lazy"
                    }
                }
            }
        }
    }))
    .unwrap_err();

    assert!(error.to_string().contains("scopes"));
}

#[test]
fn agent_only_service_has_no_implicit_store_scope() {
    let config: McpConfig = serde_json::from_value(json!({
        "mcpServers": {
            "private": {
                "command": "private-mcp",
                "_mcpstore": {
                    "scopes": {
                        "agents": {
                            "agent1": {
                                "config": {
                                    "env": {
                                        "TOKEN": "agent-token"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
    .unwrap();

    let scopes = config.mcp_servers["private"].scopes();
    assert!(scopes.store.is_none());
    assert!(scopes.agents.contains_key("agent1"));
}

#[test]
fn effective_config_applies_scope_override_and_null_deletion() {
    let config: McpConfig = serde_json::from_value(json!({
        "mcpServers": {
            "demo": {
                "command": "demo",
                "args": ["base"],
                "env": {
                    "TOKEN": "store-token",
                    "REGION": "default"
                },
                "_mcpstore": {
                    "scopes": {
                        "store": {},
                        "agents": {
                            "agent1": {
                                "config": {
                                    "args": [],
                                    "env": {
                                        "TOKEN": null
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
    .unwrap();

    let scope = ScopeRef::Agent {
        agent_id: "agent1".to_string(),
    };
    let effective = config.mcp_servers["demo"].effective_config(&scope).unwrap();
    assert!(!effective.contains_key("_mcpstore"));
    let transport = config.mcp_servers["demo"].transport_config(&scope).unwrap();
    assert!(transport.mcpstore.is_none());
    assert_eq!(
        Value::Object(effective),
        json!({
            "command": "demo",
            "args": [],
            "env": {
                "REGION": "default"
            }
        })
    );
}

#[test]
fn save_allows_structurally_valid_but_unconnectable_config() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    let path = dir.join("mcp.json");
    let mgr = ConfigManager::with_path(&path);
    let config: McpConfig = serde_json::from_value(json!({
        "mcpServers": {
            "incomplete": {
                "_mcpstore": {
                    "scopes": {
                        "store": {
                            "config": {
                                "command": null
                            }
                        }
                    }
                }
            }
        }
    }))
    .unwrap();

    mgr.save(&config).unwrap();
    assert!(mgr.load().unwrap().mcp_servers.contains_key("incomplete"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn malformed_existing_config_is_not_replaced_with_empty_config() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mcp.json");
    std::fs::write(&path, "{ malformed").unwrap();
    let mgr = ConfigManager::with_path(&path);

    assert!(mgr.load_or_empty().is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_app_config_roundtrip() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mgr = ConfigManager::with_path(dir.join("mcp.json"));

    let mut config = AppConfig::default();
    config.cache.backend = CacheBackend::Redis;
    config.cache.redis_url = Some("redis://127.0.0.1/".to_string());
    config.server.log_level = "debug".to_string();
    config.server.url_prefix = "/demo".to_string();
    config.ui.language = "en".to_string();
    config.ui.default_backup_dir = "./snapshots".to_string();
    config.ui.logging.max_size_bytes = 8 * 1024 * 1024;
    config.ui.logging.retention_days = Some(14);

    mgr.save_app_config(&config).unwrap();
    let loaded = mgr.load_app_config().unwrap();
    assert_eq!(loaded.cache.backend, CacheBackend::Redis);
    assert_eq!(loaded.cache.namespace, "mcpstore");
    assert_eq!(loaded.server.log_level, "debug");
    assert_eq!(loaded.server.url_prefix, "/demo");
    assert_eq!(loaded.ui.language, "en");
    assert_eq!(loaded.ui.default_backup_dir, "./snapshots");
    assert_eq!(loaded.ui.logging.max_size_bytes, 8 * 1024 * 1024);
    assert_eq!(loaded.ui.logging.retention_days, Some(14));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_default_template_contains_runtime_sections() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    let mgr = ConfigManager::with_path(dir.join("mcp.json"));
    let template = mgr.default_app_config_toml().unwrap();

    assert!(template.contains("[server]"));
    assert!(template.contains("[health_check]"));
    assert!(template.contains("[monitoring]"));
    assert!(template.contains("[service_defaults.lifecycle]"));
    assert!(template.contains("startup_policy = \"lazy\""));
    assert!(template.contains("restart_policy = \"no\""));
    assert!(template.contains("[standalone]"));
    assert!(template.contains("[ui]"));
    assert!(template.contains("language = \"en\""));
    assert!(template.contains("default_backup_dir = \"./backups\""));
    assert!(template.contains("[ui.logging]"));
    assert!(template.contains("max_size_bytes = 5242880"));
    assert!(template.contains("log_level = \"info\""));
}

#[test]
fn test_flatten_raw_app_config_only_contains_explicit_keys() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mgr = ConfigManager::with_path(dir.join("mcp.json"));
    std::fs::write(
        mgr.app_config_path(),
        "[server]\nhost = \"127.0.0.1\"\n[health_check]\nstartup_interval = 2.5\n",
    )
    .unwrap();

    let flattened = mgr.flatten_raw_app_config().unwrap();
    assert_eq!(
        flattened.get("server.host"),
        Some(&Value::String("127.0.0.1".to_string()))
    );
    assert_eq!(
        flattened.get("health_check.startup_interval"),
        Some(&serde_json::json!(2.5))
    );
    assert!(!flattened.contains_key("server.port"));
}

#[test]
fn test_invalid_app_config_fails_fast() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mgr = ConfigManager::with_path(dir.join("mcp.json"));
    std::fs::write(
        mgr.app_config_path(),
        "[health_check]\nstartup_interval = 0.0\n",
    )
    .unwrap();

    let err = mgr.load_app_config().unwrap_err();
    assert!(err.to_string().contains("health_check.startup_interval"));
}

#[test]
fn test_init_splits_mcp_json_and_app_toml() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    let mgr = ConfigManager::with_path(dir.join("mcp.json"));

    mgr.init(true, Some("redis://127.0.0.1/".to_string()))
        .unwrap();

    let mcp_raw = std::fs::read_to_string(mgr.mcp_path()).unwrap();
    assert!(mcp_raw.contains("mcpServers"));
    assert!(!mcp_raw.contains("cache"));

    let app_config = mgr.load_app_config().unwrap();
    assert_eq!(app_config.cache.backend, CacheBackend::Redis);
    assert_eq!(
        app_config.cache.redis_url.as_deref(),
        Some("redis://127.0.0.1/")
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_infer_transport() {
    let http = ServerConfig {
        url: Some("http://x".into()),
        ..default_server_config()
    };
    assert_eq!(http.infer_transport(), "streamable-http");

    let stdio = ServerConfig {
        command: Some("python".into()),
        ..default_server_config()
    };
    assert_eq!(stdio.infer_transport(), "stdio");
}

#[test]
fn test_service_lifecycle_extension_roundtrip_and_defaults() {
    let raw = r#"
    {
      "command": "node",
      "args": ["server.js"],
      "_mcpstore": {
        "scopes": {
          "store": {
            "config": {}
          },
          "agents": {}
        },
        "lifecycle": {
          "startup_policy": "on-store-start",
          "restart_policy": "on-failure:3"
        }
      }
    }
    "#;
    let config: ServerConfig = serde_json::from_str(raw).unwrap();
    let lifecycle = config.resolved_lifecycle(&ServiceLifecycleDefaults::default());

    assert_eq!(lifecycle.startup_policy, StartupPolicy::OnStoreStart);
    assert_eq!(lifecycle.restart_policy.kind, RestartPolicyKind::OnFailure);
    assert_eq!(lifecycle.restart_policy.max_retries, Some(3));

    let serialized = serde_json::to_value(config).unwrap();
    assert_eq!(
        serialized["_mcpstore"]["lifecycle"]["restart_policy"],
        serde_json::json!("on-failure:3")
    );
}

#[test]
fn scope_lifecycle_overrides_definition_fields_independently() {
    let config: ServerConfig = serde_json::from_value(json!({
        "command": "node",
        "_mcpstore": {
            "lifecycle": {
                "startup_policy": "on-store-start",
                "restart_policy": "on-failure:3"
            },
            "scopes": {
                "store": {
                    "config": {},
                    "lifecycle": {
                        "startup_policy": "manual"
                    }
                },
                "agents": {
                    "agent1": {
                        "config": {},
                        "lifecycle": {
                            "restart_policy": "always"
                        }
                    },
                    "agent2": {
                        "config": {}
                    }
                }
            }
        }
    }))
    .unwrap();
    let defaults = ServiceLifecycleDefaults::default();

    let store = config.resolved_lifecycle_for_scope(&ScopeRef::Store, &defaults);
    assert_eq!(store.startup_policy, StartupPolicy::Manual);
    assert_eq!(store.restart_policy.kind, RestartPolicyKind::OnFailure);
    assert_eq!(store.restart_policy.max_retries, Some(3));

    let agent1 = config.resolved_lifecycle_for_scope(
        &ScopeRef::Agent {
            agent_id: "agent1".to_string(),
        },
        &defaults,
    );
    assert_eq!(agent1.startup_policy, StartupPolicy::OnStoreStart);
    assert_eq!(agent1.restart_policy.kind, RestartPolicyKind::Always);
    assert_eq!(agent1.restart_policy.max_retries, None);

    let agent2 = config.resolved_lifecycle_for_scope(
        &ScopeRef::Agent {
            agent_id: "agent2".to_string(),
        },
        &defaults,
    );
    assert_eq!(agent2.startup_policy, StartupPolicy::OnStoreStart);
    assert_eq!(agent2.restart_policy.kind, RestartPolicyKind::OnFailure);
    assert_eq!(agent2.restart_policy.max_retries, Some(3));
}

#[test]
fn test_service_lifecycle_ignores_old_draft_field_names() {
    let raw = r#"
    {
      "command": "node",
      "startup": "on-store-start",
      "restart": "always"
    }
    "#;
    let config: ServerConfig = serde_json::from_str(raw).unwrap();
    let lifecycle = config.resolved_lifecycle(&ServiceLifecycleDefaults::default());

    assert_eq!(lifecycle.startup_policy, StartupPolicy::Lazy);
    assert_eq!(lifecycle.restart_policy.kind, RestartPolicyKind::No);
}

#[test]
fn test_app_config_service_lifecycle_defaults_from_toml() {
    let config: AppConfig = toml::from_str(
        r#"
        [service_defaults.lifecycle]
        startup_policy = "on-store-start"
        restart_policy = "unless-stopped"
        "#,
    )
    .unwrap();

    assert_eq!(
        config.service_defaults.lifecycle.startup_policy,
        StartupPolicy::OnStoreStart
    );
    assert_eq!(
        config.service_defaults.lifecycle.restart_policy.kind,
        RestartPolicyKind::UnlessStopped
    );
}

#[test]
fn test_add_examples_only_inserts_missing_entries() {
    let dir = std::env::temp_dir().join(format!("mcpstore_test_{}", uuid::Uuid::new_v4()));
    let path = dir.join("mcp.json");
    let mgr = ConfigManager::with_path(&path);

    let mut config = McpConfig::default();
    config.mcp_servers.insert(
        "remote-http-service".to_string(),
        ServerConfig {
            url: Some("https://override.example.com/mcp".to_string()),
            transport: Some("streamable-http".to_string()),
            description: Some("Existing service".to_string()),
            ..default_server_config()
        },
    );
    config.mcp_servers.insert(
        "custom-service".to_string(),
        ServerConfig {
            command: Some("echo".to_string()),
            args: vec!["ok".to_string()],
            transport: Some("stdio".to_string()),
            ..default_server_config()
        },
    );

    mgr.save(&config).unwrap();
    let added = mgr.add_examples().unwrap();
    let loaded = mgr.load().unwrap();

    assert_eq!(added, 2);
    assert_eq!(loaded.mcp_servers.len(), 4);
    assert_eq!(
        loaded
            .mcp_servers
            .get("remote-http-service")
            .and_then(|svc| svc.url.as_deref()),
        Some("https://override.example.com/mcp")
    );
    assert!(loaded.mcp_servers.contains_key("local-command-service"));
    assert!(loaded.mcp_servers.contains_key("npm-package-service"));
    assert!(loaded.mcp_servers.contains_key("custom-service"));

    std::fs::remove_dir_all(&dir).ok();
}
