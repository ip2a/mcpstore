use std::sync::{atomic::AtomicU64, RwLock as SyncRwLock};

use crate::cache::models::{
    AgentServiceRelation, HealthStatus, ServiceEntity, ServiceRelationItem, ServiceStatus,
    ServiceToolRelation, ToolAvailability, ToolEntity, ToolRelationItem, ToolStatusItem,
};
use crate::cache::CacheLayerManager;
use crate::config::{CacheBackend, ConfigManager, ServerConfig};
use crate::events::{Event, EventBus};
use crate::registry::{ConnectionStatus, ServiceEntry, ServiceRegistry};
use crate::transport::client::ConnectionPool;
use crate::transport::ToolDescription;

use crate::perspective::{
    generate_tool_global_name, normalize_service_name, parse_agent_scoped, resolve_tool,
    AvailableTool, ToolResolution, GLOBAL_AGENT_STORE,
};
use crate::{Result, StoreError};

mod agent_scope;
mod cache_admin;
mod control_queue;
mod db_refresh;
mod health;
mod payload;
mod scoped_content;
mod scoped_views;
mod types;
use types::StoreRuntimeConfig;

pub use types::{
    BackendKind, CacheHealthReport, CacheStorage, EventCapabilityReport, ScopedServiceEntry,
    ScopedServiceHealth, ScopedToolEntry, SourceMode, StoreOptions,
};

const ONLYDB_CONTROL_EVENT_TYPE: &str = "control_requests";
static CONTROL_EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn merge_json_object(target: &mut serde_json::Value, updates: serde_json::Value) -> Result<()> {
    let target_object = target.as_object_mut().ok_or_else(|| {
        StoreError::Other("Service config is not a JSON object, cannot patch".to_string())
    })?;
    let updates_object = updates.as_object().ok_or_else(|| {
        StoreError::Other("Service config patch must be a JSON object".to_string())
    })?;

    for (key, value) in updates_object {
        match (target_object.get_mut(key), value) {
            (Some(existing @ serde_json::Value::Object(_)), serde_json::Value::Object(_)) => {
                merge_json_object(existing, value.clone())?;
            }
            _ => {
                target_object.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(())
}

pub struct MCPStore {
    config_manager: ConfigManager,
    source_mode: SourceMode,
    runtime_config: StoreRuntimeConfig,
    cache_storage: tokio::sync::RwLock<CacheStorage>,
    redis_url: tokio::sync::RwLock<Option<String>>,
    namespace: SyncRwLock<String>,
    registry: ServiceRegistry,
    pool: ConnectionPool,
    event_bus: EventBus,
    cache: CacheLayerManager,
}

impl MCPStore {
    pub fn setup(config_path: Option<&str>) -> Result<Self> {
        Self::setup_with_options(StoreOptions {
            config_path: config_path.map(ToString::to_string),
            ..StoreOptions::default()
        })
    }

    pub fn setup_with_options(options: StoreOptions) -> Result<Self> {
        let config_manager = match options.config_path.as_deref() {
            Some(p) => ConfigManager::with_path(p),
            None => ConfigManager::new(),
        };

        let app_config = config_manager.load_app_config_or_default()?;
        let runtime_config = StoreRuntimeConfig::from_app_config(&app_config);
        let namespace = options
            .namespace
            .clone()
            .unwrap_or_else(|| app_config.cache.namespace.clone());
        let cache_storage = options
            .backend
            .clone()
            .unwrap_or(match &app_config.cache.backend {
                CacheBackend::Redis => CacheStorage::Redis,
                CacheBackend::Memory => CacheStorage::Memory,
                CacheBackend::OpenKeyvMemory => CacheStorage::OpenKeyvMemory,
                CacheBackend::OpenKeyvRedis => CacheStorage::OpenKeyvRedis,
            });
        let redis_url = options
            .redis_url
            .clone()
            .or_else(|| app_config.cache.redis_url.clone())
            .unwrap_or_else(|| "redis://127.0.0.1/".to_string());
        let cache_store = Self::build_backend(&cache_storage, &redis_url, &namespace)?;

        Ok(Self {
            config_manager,
            source_mode: options.source_mode,
            runtime_config,
            cache_storage: tokio::sync::RwLock::new(cache_storage),
            redis_url: tokio::sync::RwLock::new(Some(redis_url)),
            namespace: SyncRwLock::new(namespace.clone()),
            registry: ServiceRegistry::new(),
            pool: ConnectionPool::new(),
            event_bus: EventBus::with_history(1000),
            cache: CacheLayerManager::new(cache_store, namespace),
        })
    }

    pub fn config_manager(&self) -> &ConfigManager {
        &self.config_manager
    }

    pub fn cache(&self) -> &CacheLayerManager {
        &self.cache
    }

    pub fn namespace(&self) -> String {
        self.namespace
            .read()
            .expect("store namespace lock poisoned")
            .clone()
    }

    pub fn source_mode(&self) -> SourceMode {
        self.source_mode
    }

    pub fn is_db_source(&self) -> bool {
        self.source_mode == SourceMode::Db
    }

    pub async fn add_service(&self, name: &str, config: ServerConfig) -> Result<()> {
        self.add_service_with_identity(name, name, GLOBAL_AGENT_STORE, config)
            .await
    }

    pub async fn add_service_for_agent(
        &self,
        agent_id: &str,
        local_name: &str,
        config: ServerConfig,
    ) -> Result<String> {
        let resolution = normalize_service_name(agent_id, local_name, "global", true)?;
        if self.source_mode == SourceMode::Db {
            self.queue_service_add_request(
                &resolution.global_name,
                &resolution.local_name,
                agent_id,
                &config,
            )
            .await?;
            return Ok(resolution.global_name);
        }
        self.add_service_with_identity(
            &resolution.global_name,
            &resolution.local_name,
            agent_id,
            config,
        )
        .await?;
        self.assign_service_to_agent(agent_id, &resolution.global_name)
            .await?;
        Ok(resolution.global_name)
    }

    async fn add_service_with_identity(
        &self,
        name: &str,
        original_name: &str,
        agent_id: &str,
        config: ServerConfig,
    ) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_service_add_request(name, original_name, agent_id, &config)
                .await;
        }

        let transport = config.infer_transport().to_string();

        let entry = ServiceEntry {
            name: name.to_string(),
            original_name: original_name.to_string(),
            agent_id: agent_id.to_string(),
            transport: transport.clone(),
            url: config.url.clone(),
            command: config.command.clone(),
            status: ConnectionStatus::Disconnected,
            tools: Vec::new(),
            config: serde_json::to_value(&config).unwrap_or_default(),
            added_time: chrono::Utc::now().timestamp(),
        };

        self.registry.register(entry).await;
        self.pool.add(name.to_string(), config.clone()).await;
        self.cache_service_added(
            name,
            original_name,
            agent_id,
            &config,
            chrono::Utc::now().timestamp(),
        )
        .await?;

        self.event_bus
            .publish(
                Event::new("SERVICE_ADD_REQUESTED", serde_json::json!({ "name": name })),
                true,
            )
            .await;

        if self.source_mode == SourceMode::Local {
            let mut cfg = self.config_manager.load_or_default();
            cfg.mcp_servers.insert(name.to_string(), config);
            self.config_manager.save(&cfg)?;
        }

        tracing::info!("[STORE] Service added: {} (transport={})", name, transport);
        Ok(())
    }

    pub async fn connect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request(
                    "ServiceConnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }
        self.connect_service_internal(name, false).await
    }

    async fn connect_service_internal(&self, name: &str, automatic_retry: bool) -> Result<()> {
        if self.registry.find_service(name).await.is_none() {
            return Err(StoreError::ServiceNotFound(name.to_string()));
        }
        if self.pool.is_connected(name).await {
            self.registry
                .update_status(name, ConnectionStatus::Connected)
                .await;
            return Ok(());
        }

        let retry_state = self.sync_retry_window(name).await?;
        let now = Self::now_timestamp_f64();
        if automatic_retry {
            if let Some(status) = retry_state.as_ref() {
                if Self::retry_exhausted(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service automatic retry exhausted: {name}"
                    )));
                }
                if let Some(retry_in_secs) = Self::retry_wait_seconds(status, now) {
                    return Err(StoreError::Other(format!(
                        "Service reconnect backoff active: {name}, retry_in={retry_in_secs}s"
                    )));
                }
            }
        }

        self.registry
            .update_status(name, ConnectionStatus::Connecting)
            .await;
        let previous_status = retry_state
            .as_ref()
            .map(|status| status.health_status.clone());
        let mut startup = retry_state.unwrap_or_else(|| {
            self.service_status_payload(name, HealthStatus::Startup, None, Vec::new())
        });
        startup.health_status = HealthStatus::Startup;
        startup.last_health_check = Self::now_timestamp();
        startup.next_retry_time = None;
        startup.lease_deadline = if matches!(previous_status, Some(HealthStatus::HalfOpen)) {
            Some(now + self.runtime_config.half_open_lease_secs as f64)
        } else {
            None
        };
        self.put_service_status_payload(&startup).await?;
        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTION_REQUESTED",
                    serde_json::json!({ "name": name }),
                ),
                true,
            )
            .await;

        if let Err(error) = self.pool.connect(name).await {
            let message = format!("Connection failed: {error}");
            self.pool.disconnect(name).await.ok();
            self.registry
                .update_status(name, ConnectionStatus::Error)
                .await;
            self.mark_service_retryable_failure(name, message.clone())
                .await?;
            return Err(error.into());
        }
        self.registry
            .update_status(name, ConnectionStatus::Connected)
            .await;

        let tools = match self.pool.list_tools(name).await {
            Ok(tools) => tools,
            Err(error) => {
                let message = format!("Tool discovery failed: {error}");
                self.pool.disconnect(name).await.ok();
                self.registry
                    .update_status(name, ConnectionStatus::Error)
                    .await;
                self.mark_service_retryable_failure(name, message.clone())
                    .await?;
                return Err(error.into());
            }
        };
        let tool_infos: Vec<crate::registry::ToolInfo> = tools
            .into_iter()
            .map(|t| crate::registry::ToolInfo {
                name: t.name,
                description: t.description,
                schema: t.input_schema,
            })
            .collect();

        let tool_count = tool_infos.len();
        if let Some(mut entry) = self.registry.find_service(name).await {
            entry.tools = tool_infos;
            entry.status = ConnectionStatus::Connected;
            self.registry.register(entry).await;
        }

        let tools = self.registry.list_service_tools(name).await;
        self.cache_service_connected(name, &tools).await?;

        self.event_bus
            .publish(
                Event::new(
                    "SERVICE_CONNECTED",
                    serde_json::json!({
                        "name": name, "tools_count": tool_count
                    }),
                ),
                true,
            )
            .await;

        tracing::info!("[STORE] Service connected: {} (tools={})", name, tool_count);
        Ok(())
    }

    pub async fn remove_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request(
                    "ServiceRemoveRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.remove(name).await.ok();
        self.registry.unregister(name).await;
        self.cache_service_removed(name).await?;

        if self.source_mode == SourceMode::Local {
            let mut cfg = self.config_manager.load_or_default();
            cfg.mcp_servers.remove(name);
            self.config_manager.save(&cfg)?;
        }

        self.event_bus
            .publish(
                Event::new("SERVICE_REMOVED", serde_json::json!({ "name": name })),
                true,
            )
            .await;

        tracing::info!("[STORE] Service removed: {}", name);
        Ok(())
    }

    pub async fn update_service(&self, name: &str, config: ServerConfig) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request(
                    "ServiceUpdateRequested",
                    serde_json::json!({
                        "service_name": name,
                        "config": config,
                    }),
                )
                .await;
        }

        let existing = self
            .registry
            .find_service(name)
            .await
            .ok_or_else(|| StoreError::ServiceNotFound(name.to_string()))?;
        self.pool.remove(name).await.ok();
        self.registry.unregister(name).await;
        self.cache_service_removed(name).await?;
        self.add_service_with_identity(name, &existing.original_name, &existing.agent_id, config)
            .await?;
        if existing.agent_id != GLOBAL_AGENT_STORE {
            self.registry
                .add_to_agent_scope(&existing.agent_id, name)
                .await;
            self.cache_agent_scope(&existing.agent_id).await?;
        }
        Ok(())
    }

    pub async fn patch_service(&self, name: &str, updates: serde_json::Value) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request(
                    "ServicePatchRequested",
                    serde_json::json!({
                        "service_name": name,
                        "updates": updates,
                    }),
                )
                .await;
        }

        let mut config = self
            .get_service_config(name)
            .await?
            .ok_or_else(|| StoreError::ServiceNotFound(name.to_string()))?;
        merge_json_object(&mut config, updates)?;
        let config: ServerConfig = serde_json::from_value(config).map_err(|e| {
            StoreError::Other(format!(
                "Post-patch service config deserialization failed: {e}"
            ))
        })?;
        self.update_service(name, config).await
    }

    pub async fn list_services(&self) -> Vec<ServiceEntry> {
        self.refresh_from_db_if_needed().await.ok();
        let mut services = self.registry.list_services().await;
        if self.source_mode == SourceMode::Db {
            for service in &mut services {
                self.hydrate_service_status_from_cache(service).await.ok();
            }
        }
        services
    }

    pub async fn find_service(&self, name: &str) -> Option<ServiceEntry> {
        self.refresh_from_db_if_needed().await.ok();
        let mut service = self.registry.find_service(name).await?;
        if self.source_mode == SourceMode::Db {
            self.hydrate_service_status_from_cache(&mut service)
                .await
                .ok();
        }
        Some(service)
    }

    pub async fn list_tools(&self, service_name: &str) -> Result<Vec<ToolDescription>> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        Ok(self
            .registry
            .list_service_tools(service_name)
            .await
            .into_iter()
            .map(|tool| ToolDescription {
                name: tool.name,
                description: tool.description,
                input_schema: tool.schema,
            })
            .collect())
    }

    pub async fn list_resources(&self, service_name: &str) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_resources(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_resource_templates(
        &self,
        service_name: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_resource_templates(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn read_resource(&self, service_name: &str, uri: &str) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .read_resource(service_name, uri)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_prompts(&self, service_name: &str) -> Result<Vec<serde_json::Value>> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .list_prompts(service_name)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn get_prompt(
        &self,
        service_name: &str,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.ensure_service_connected(service_name).await?;
        self.pool
            .get_prompt(service_name, prompt_name, arguments)
            .await
            .map_err(StoreError::Transport)
    }

    pub async fn list_all_tools(&self) -> Vec<crate::registry::ToolInfo> {
        self.refresh_from_db_if_needed().await.ok();
        self.registry.list_all_tools().await
    }

    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        let mut agent_ids = self.registry.list_agent_ids().await;
        let cached = self.cache.get_all_relations_async("agent_services").await?;
        for agent_id in cached.keys() {
            if !agent_ids.contains(agent_id) {
                agent_ids.push(agent_id.clone());
            }
        }
        agent_ids.sort();
        agent_ids.dedup();

        let mut agents = Vec::with_capacity(agent_ids.len());
        for agent_id in agent_ids {
            agents.push(serde_json::json!({
                "agent_id": agent_id,
                "services": self.list_agent_service_names(&agent_id).await?,
            }));
        }
        Ok(agents)
    }

    pub async fn call_tool(
        &self,
        service_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<crate::transport::ToolCallResult> {
        self.refresh_from_db_if_needed().await?;
        if self.registry.find_service(service_name).await.is_none() {
            return Err(StoreError::ServiceNotFound(service_name.to_string()));
        }
        if !self.pool.is_connected(service_name).await {
            self.connect_service_internal(service_name, true).await?;
        }
        let event_args = args.clone();
        let started_at = std::time::Instant::now();
        match self.pool.call_tool(service_name, tool_name, args).await {
            Ok(result) => {
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.record_health_check_result(service_name, true, Some(latency_ms), None)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_COMPLETED",
                            serde_json::json!({
                                "service_name": service_name,
                                "tool_name": tool_name,
                                "arguments": event_args,
                                "latency_ms": latency_ms,
                                "is_error": result.is_error,
                                "status": if result.is_error { "error" } else { "success" },
                            }),
                        ),
                        true,
                    )
                    .await;
                Ok(result)
            }
            Err(error) => {
                let message = format!("Tool call failed: {error}");
                let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                self.pool.disconnect(service_name).await.ok();
                self.registry
                    .update_status(service_name, ConnectionStatus::Error)
                    .await;
                self.mark_service_retryable_failure(service_name, message)
                    .await?;
                self.event_bus
                    .publish(
                        Event::new(
                            "TOOL_CALL_FAILED",
                            serde_json::json!({
                                "service_name": service_name,
                                "tool_name": tool_name,
                                "arguments": event_args,
                                "latency_ms": latency_ms,
                                "is_error": true,
                                "status": "error",
                                "error": error.to_string(),
                            }),
                        ),
                        true,
                    )
                    .await;
                Err(StoreError::Transport(error))
            }
        }
    }

    pub async fn resolve_tool_for_agent(
        &self,
        agent_id: &str,
        user_input: &str,
    ) -> Result<ToolResolution> {
        self.refresh_from_db_if_needed().await?;
        let service_names = if agent_id == GLOBAL_AGENT_STORE {
            self.registry
                .list_services()
                .await
                .into_iter()
                .map(|service| service.name)
                .collect::<Vec<_>>()
        } else {
            self.list_agent_service_names(agent_id).await?
        };
        let mut available = Vec::new();
        for global_service_name in service_names {
            let service = match self.registry.find_service(&global_service_name).await {
                Some(service) => service,
                None => continue,
            };
            let local_service_name = if agent_id == GLOBAL_AGENT_STORE {
                service.name.clone()
            } else {
                service.original_name.clone()
            };
            for tool in service.tools {
                let global_tool_name = generate_tool_global_name(&service.name, &tool.name)?;
                available.push(AvailableTool {
                    name: format!("{}_{}", local_service_name, tool.name),
                    original_name: Some(tool.name),
                    service_name: Some(local_service_name.clone()),
                    global_service_name: Some(service.name.clone()),
                    global_tool_name: Some(global_tool_name),
                });
            }
        }
        resolve_tool(agent_id, user_input, &available, "canonical", true)
    }

    pub async fn disconnect_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request(
                    "ServiceDisconnectRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.pool.disconnect(name).await?;
        self.registry
            .update_status(name, ConnectionStatus::Disconnected)
            .await;
        self.set_service_status(name, HealthStatus::Disconnected, None, Vec::new())
            .await?;
        self.event_bus
            .publish(
                Event::new("SERVICE_DISCONNECTED", serde_json::json!({ "name": name })),
                true,
            )
            .await;
        tracing::info!("[STORE] Service disconnected: {}", name);
        Ok(())
    }

    pub async fn show_config(&self) -> Result<serde_json::Value> {
        let config = self.show_config_entry().await?;
        serde_json::to_value(config)
            .map_err(|e| StoreError::Other(format!("Config serialization failed: {e}")))
    }

    pub async fn show_config_entry(&self) -> Result<crate::config::McpConfig> {
        if self.source_mode == SourceMode::Db {
            self.refresh_from_db_if_needed().await?;

            let mut config = crate::config::McpConfig::default();
            for service in self.registry.list_services().await {
                let service_config: ServerConfig = serde_json::from_value(service.config.clone())
                    .map_err(|e| {
                    StoreError::Other(format!(
                        "Service config deserialization failed during show_config: {e}"
                    ))
                })?;
                config.mcp_servers.insert(service.name, service_config);
            }

            let mut agent_ids = self.registry.list_agent_ids().await;
            agent_ids.sort();
            agent_ids.dedup();
            for agent_id in agent_ids {
                if agent_id == GLOBAL_AGENT_STORE {
                    continue;
                }
                let mut services = self.registry.list_agent_services(&agent_id).await;
                services.sort();
                services.dedup();
                if !services.is_empty() {
                    config.agents.insert(agent_id, services);
                }
            }

            return Ok(config);
        }

        Ok(self.config_manager.load_or_default())
    }

    pub async fn reset_config(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request("StoreResetRequested", serde_json::json!({}))
                .await;
        }

        self.pool.clear().await;
        self.registry.clear().await;
        let snapshot = self.cache.snapshot().await?;
        for (entity_type, entries) in snapshot.entities {
            for key in entries.keys() {
                self.cache.delete_entity(&entity_type, key).await?;
            }
        }
        for (relation_type, entries) in snapshot.relations {
            for key in entries.keys() {
                self.cache.delete_relation(&relation_type, key).await?;
            }
        }
        for (state_type, entries) in snapshot.states {
            for key in entries.keys() {
                self.cache.delete_state(&state_type, key).await?;
            }
        }
        for (event_type, entries) in snapshot.events {
            for key in entries.keys() {
                self.cache.delete_event(&event_type, key).await?;
            }
        }

        if self.source_mode == SourceMode::Local {
            self.config_manager
                .save(&crate::config::McpConfig::default())?;
        }
        Ok(())
    }

    pub async fn load_from_config(&self) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self.load_from_db().await;
        }

        let cfg = self.config_manager.load_or_default();
        for (name, server_config) in cfg.mcp_servers {
            if self.registry.find_service(&name).await.is_none() {
                self.register_configured_service(&name, server_config)
                    .await?;
            }
        }
        let cfg = self.config_manager.load_or_default();
        for (agent_id, services) in cfg.agents {
            for service_name in services {
                if self.registry.find_service(&service_name).await.is_some() {
                    self.registry
                        .add_to_agent_scope(&agent_id, &service_name)
                        .await;
                }
            }
            self.cache_agent_scope(&agent_id).await?;
        }
        Ok(())
    }

    pub async fn load_from_source(&self) -> Result<()> {
        self.load_from_config().await
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return self
                .queue_onlydb_control_request(
                    "ServiceRestartRequested",
                    serde_json::json!({ "service_name": name }),
                )
                .await;
        }

        self.disconnect_service(name).await.ok();
        self.connect_service(name).await
    }

    pub async fn get_service_config(&self, name: &str) -> Result<Option<serde_json::Value>> {
        self.refresh_from_db_if_needed().await?;
        if let Some(entry) = self.registry.find_service(name).await {
            return Ok(Some(entry.config));
        }
        let Some(value) = self.cache.get_entity("services", name).await? else {
            return Ok(None);
        };
        Ok(value.get("config").cloned())
    }

    async fn cache_service_added(
        &self,
        name: &str,
        original_name: &str,
        agent_id: &str,
        config: &ServerConfig,
        now: i64,
    ) -> Result<()> {
        let entity = ServiceEntity {
            service_global_name: name.to_string(),
            service_original_name: original_name.to_string(),
            source_agent: agent_id.to_string(),
            config: serde_json::to_value(config).unwrap_or_default(),
            added_time: now,
        };
        self.cache
            .put_entity(
                "services",
                name,
                serde_json::to_value(entity).unwrap_or_default(),
            )
            .await?;

        self.upsert_agent_service_relation(agent_id, name, now)
            .await?;
        self.set_service_status(name, HealthStatus::Init, None, Vec::new())
            .await?;
        self.cache
            .put_event(
                "service",
                &format!("{name}:added:{now}"),
                serde_json::json!({
                    "event": "service_added",
                    "service": name,
                    "timestamp": now
                }),
            )
            .await?;
        Ok(())
    }

    async fn register_configured_service(&self, name: &str, config: ServerConfig) -> Result<()> {
        let parsed = parse_agent_scoped(name)?;
        let agent_id = parsed
            .agent_id
            .unwrap_or_else(|| GLOBAL_AGENT_STORE.to_string());
        let original_name = parsed.local_name;
        let transport = config.infer_transport().to_string();
        let entry = ServiceEntry {
            name: name.to_string(),
            original_name: original_name.clone(),
            agent_id: agent_id.clone(),
            transport,
            url: config.url.clone(),
            command: config.command.clone(),
            status: ConnectionStatus::Disconnected,
            tools: Vec::new(),
            config: serde_json::to_value(&config).unwrap_or_default(),
            added_time: chrono::Utc::now().timestamp(),
        };
        self.registry.register(entry).await;
        self.pool.add(name.to_string(), config.clone()).await;
        if self.cache.get_entity("services", name).await?.is_none() {
            self.cache_service_added(
                name,
                &original_name,
                &agent_id,
                &config,
                chrono::Utc::now().timestamp(),
            )
            .await?;
        }
        if agent_id != GLOBAL_AGENT_STORE {
            self.registry.add_to_agent_scope(&agent_id, name).await;
            if self.source_mode == SourceMode::Local {
                self.cache_agent_scope(&agent_id).await?;
            }
        }
        Ok(())
    }

    async fn cache_agent_scope(&self, agent_id: &str) -> Result<()> {
        let service_names = self.registry.list_agent_services(agent_id).await;
        self.cache_agent_scope_names(agent_id, service_names).await
    }

    async fn cache_agent_scope_names(
        &self,
        agent_id: &str,
        service_names: Vec<String>,
    ) -> Result<()> {
        let mut services = Vec::with_capacity(service_names.len());
        let now = chrono::Utc::now().timestamp();
        for service_name in service_names {
            let parsed = parse_agent_scoped(&service_name)?;
            services.push(ServiceRelationItem {
                service_original_name: parsed.local_name,
                service_global_name: service_name.clone(),
                client_id: service_name,
                established_time: now,
                last_access: Some(now),
            });
        }
        self.cache
            .put_relation(
                "agent_services",
                agent_id,
                serde_json::to_value(AgentServiceRelation { services }).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    async fn upsert_agent_service_relation(
        &self,
        agent_id: &str,
        service_name: &str,
        now: i64,
    ) -> Result<()> {
        let mut relation = match self.cache.get_relation("agent_services", agent_id).await? {
            Some(value) => serde_json::from_value(value).map_err(|e| {
                StoreError::Other(format!("Agent relation deserialization failed: {e}"))
            })?,
            None => AgentServiceRelation {
                services: Vec::new(),
            },
        };

        if !relation
            .services
            .iter()
            .any(|item| item.service_global_name == service_name)
        {
            let parsed = parse_agent_scoped(service_name)?;
            relation.services.push(ServiceRelationItem {
                service_original_name: parsed.local_name,
                service_global_name: service_name.to_string(),
                client_id: service_name.to_string(),
                established_time: now,
                last_access: Some(now),
            });
        }

        self.cache
            .put_relation(
                "agent_services",
                agent_id,
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        Ok(())
    }

    async fn cached_agent_scope(&self, agent_id: &str) -> Result<Vec<String>> {
        let value = self.cache.get_relation("agent_services", agent_id).await?;
        match value {
            Some(value) => {
                let relation: AgentServiceRelation =
                    serde_json::from_value(value).map_err(|e| {
                        StoreError::Other(format!("Agent scope deserialization failed: {e}"))
                    })?;
                Ok(relation
                    .services
                    .into_iter()
                    .map(|item| item.service_global_name)
                    .collect())
            }
            None => Ok(Vec::new()),
        }
    }

    async fn cache_service_connected(
        &self,
        name: &str,
        tools: &[crate::registry::ToolInfo],
    ) -> Result<()> {
        if self.source_mode == SourceMode::Db {
            return Ok(());
        }

        let now = chrono::Utc::now().timestamp();
        let parsed = parse_agent_scoped(name)?;
        let source_agent = parsed
            .agent_id
            .unwrap_or_else(|| GLOBAL_AGENT_STORE.to_string());
        let service_original_name = parsed.local_name;
        let mut relation_tools = Vec::with_capacity(tools.len());
        let mut status_tools = Vec::with_capacity(tools.len());
        for tool in tools {
            let global_name = generate_tool_global_name(name, &tool.name)?;
            let entity = ToolEntity {
                tool_global_name: global_name.clone(),
                tool_original_name: tool.name.clone(),
                service_global_name: name.to_string(),
                service_original_name: service_original_name.clone(),
                source_agent: source_agent.clone(),
                description: tool.description.clone(),
                input_schema: tool.schema.clone(),
                created_time: now,
                tool_hash: format!("{}:{}:{}", name, tool.name, now),
            };
            self.cache
                .put_entity(
                    "tools",
                    &global_name,
                    serde_json::to_value(entity).unwrap_or_default(),
                )
                .await?;
            relation_tools.push(ToolRelationItem {
                tool_global_name: global_name.clone(),
                tool_original_name: tool.name.clone(),
            });
            status_tools.push(ToolStatusItem {
                tool_global_name: global_name,
                tool_original_name: tool.name.clone(),
                status: ToolAvailability::Available,
            });
        }

        let relation = ServiceToolRelation {
            service_global_name: name.to_string(),
            service_original_name,
            source_agent,
            tools: relation_tools,
        };
        self.cache
            .put_relation(
                "service_tools",
                name,
                serde_json::to_value(relation).unwrap_or_default(),
            )
            .await?;
        self.set_service_status(name, HealthStatus::Healthy, None, status_tools)
            .await?;
        self.cache
            .put_event(
                "service",
                &format!("{name}:connected:{now}"),
                serde_json::json!({
                    "event": "service_connected",
                    "service": name,
                    "timestamp": now,
                    "tools_count": tools.len()
                }),
            )
            .await?;
        Ok(())
    }

    async fn cache_service_removed(&self, name: &str) -> Result<()> {
        self.cache.delete_entity("services", name).await?;
        self.cache.delete_relation("service_tools", name).await?;
        self.cache.delete_state("service_status", name).await?;
        self.remove_service_from_agent_relations(name).await?;
        let now = chrono::Utc::now().timestamp();
        self.cache
            .put_event(
                "service",
                &format!("{name}:removed:{now}"),
                serde_json::json!({
                    "event": "service_removed",
                    "service": name,
                    "timestamp": now
                }),
            )
            .await?;
        Ok(())
    }

    async fn remove_service_from_agent_relations(&self, name: &str) -> Result<()> {
        let relations = self.cache.get_all_relations_async("agent_services").await?;
        for (agent_id, value) in relations {
            let mut relation: AgentServiceRelation =
                serde_json::from_value(value).map_err(|e| {
                    StoreError::Other(format!("Agent relation deserialization failed: {e}"))
                })?;
            let original_len = relation.services.len();
            relation.services.retain(|item| {
                item.service_global_name != name && item.service_original_name != name
            });

            if relation.services.is_empty() {
                self.cache
                    .delete_relation("agent_services", &agent_id)
                    .await?;
            } else if relation.services.len() != original_len {
                self.cache
                    .put_relation(
                        "agent_services",
                        &agent_id,
                        serde_json::to_value(relation).unwrap_or_default(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn temp_config_path() -> String {
        std::env::temp_dir()
            .join(format!("mcpstore-store-{}.json", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .to_string()
    }

    fn stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("fixture".to_string()),
        }
    }

    fn broken_stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("__mcpstore_missing_binary__".to_string()),
            args: Vec::new(),
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("broken".to_string()),
        }
    }

    #[tokio::test]
    async fn add_service_writes_cache_layers() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let entity = store.cache().get_entity("services", "svc").await.unwrap();
        assert!(entity.is_some());

        let relation = store
            .cache()
            .get_relation("agent_services", "global_agent_store")
            .await
            .unwrap();
        assert!(relation.is_some());

        let status = store
            .cache()
            .get_state("service_status", "svc")
            .await
            .unwrap();
        assert!(status.is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn add_service_for_agent_uses_global_identity() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        let global_name = store
            .add_service_for_agent("agent-a", "svc", stdio_config())
            .await
            .unwrap();

        assert_eq!(global_name, "svc_byagent_agent-a");
        let entry = store.find_service(&global_name).await.unwrap();
        assert_eq!(entry.original_name, "svc");
        assert_eq!(entry.agent_id, "agent-a");
        assert_eq!(
            store.list_agent_service_names("agent-a").await.unwrap(),
            vec![global_name.clone()]
        );

        let entity = store
            .cache()
            .get_entity("services", &global_name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entity["service_original_name"], "svc");
        assert_eq!(entity["source_agent"], "agent-a");

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn remove_service_clears_service_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();
        store.remove_service("svc").await.unwrap();

        let entity = store.cache().get_entity("services", "svc").await.unwrap();
        assert!(entity.is_none());

        let status = store
            .cache()
            .get_state("service_status", "svc")
            .await
            .unwrap();
        assert!(status.is_none());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn db_source_does_not_write_config_file() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source".to_string()),
        })
        .unwrap();

        store.add_service("svc", stdio_config()).await.unwrap();

        assert!(!std::path::Path::new(&path).exists());
        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_none());
        let events = store
            .cache()
            .get_all_events_async(ONLYDB_CONTROL_EVENT_TYPE)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        let event = events.values().next().unwrap();
        assert_eq!(event["type"], serde_json::json!("ServiceAddRequested"));
        assert_eq!(event["status"], serde_json::json!("pending"));
        assert_eq!(event["payload"]["service_name"], serde_json::json!("svc"));
    }

    #[tokio::test]
    async fn db_source_refreshes_cache_on_scoped_reads() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-refresh".to_string()),
        })
        .unwrap();
        let config = stdio_config();

        assert!(store.list_services_scoped(None).await.unwrap().is_empty());
        store
            .cache()
            .put_entity(
                "services",
                "svc",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    config: serde_json::to_value(config).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_entity(
                "tools",
                "svc_echo",
                serde_json::to_value(ToolEntity {
                    tool_global_name: "svc_echo".to_string(),
                    tool_original_name: "echo".to_string(),
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    description: "echo tool".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    created_time: 111,
                    tool_hash: "fixture".to_string(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "service_tools",
                "svc",
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    tools: vec![ToolRelationItem {
                        tool_global_name: "svc_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();

        let services = store.list_services_scoped(None).await.unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0]["name"], serde_json::json!("svc"));
        let tools = store.list_tools_scoped(None, None).await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], serde_json::json!("svc_echo"));

        store
            .cache()
            .delete_entity("services", "svc")
            .await
            .unwrap();
        let services = store.list_services_scoped(None).await.unwrap();
        assert!(services.is_empty());
    }

    #[tokio::test]
    async fn db_source_refreshes_public_reads_and_show_config() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-public-reads".to_string()),
        })
        .unwrap();
        let config = stdio_config();

        assert!(store.list_services().await.is_empty());
        assert!(store.find_service("svc").await.is_none());
        assert!(store.list_all_tools().await.is_empty());

        store
            .cache()
            .put_entity(
                "services",
                "svc",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    config: serde_json::to_value(config.clone()).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_entity(
                "tools",
                "svc_echo",
                serde_json::to_value(ToolEntity {
                    tool_global_name: "svc_echo".to_string(),
                    tool_original_name: "echo".to_string(),
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    description: "echo tool".to_string(),
                    input_schema: serde_json::json!({"type": "object"}),
                    created_time: 111,
                    tool_hash: "fixture".to_string(),
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "service_tools",
                "svc",
                serde_json::to_value(ServiceToolRelation {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    tools: vec![ToolRelationItem {
                        tool_global_name: "svc_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "agent_services",
                "agent-a",
                serde_json::to_value(AgentServiceRelation {
                    services: vec![ServiceRelationItem {
                        service_global_name: "svc".to_string(),
                        service_original_name: "svc".to_string(),
                        established_time: 111,
                        last_access: Some(111),
                        client_id: "svc".to_string(),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_state(
                "service_status",
                "svc",
                serde_json::to_value(store.service_status_payload(
                    "svc",
                    HealthStatus::Healthy,
                    None,
                    vec![ToolStatusItem {
                        tool_global_name: "svc_echo".to_string(),
                        tool_original_name: "echo".to_string(),
                        status: ToolAvailability::Available,
                    }],
                ))
                .unwrap(),
            )
            .await
            .unwrap();

        let services = store.list_services().await;
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "svc");
        assert_eq!(services[0].status, ConnectionStatus::Connected);

        let service = store.find_service("svc").await.unwrap();
        assert_eq!(service.original_name, "svc");
        assert_eq!(service.status, ConnectionStatus::Connected);

        let tools = store.list_all_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        let scoped_services = store.list_services_scoped(None).await.unwrap();
        assert_eq!(scoped_services[0]["status"], serde_json::json!("connected"));
        let scoped_service = store.service_info_scoped(None, "svc").await.unwrap();
        assert_eq!(scoped_service["status"], serde_json::json!("connected"));

        let config_view = store.show_config().await.unwrap();
        assert_eq!(
            config_view["mcpServers"]["svc"]["args"],
            serde_json::json!(["fixture"])
        );
        assert_eq!(config_view["agents"]["agent-a"], serde_json::json!(["svc"]));
        assert!(config_view.get("global_agent_store").is_none());

        store
            .cache()
            .delete_entity("services", "svc")
            .await
            .unwrap();
        assert!(store.find_service("svc").await.is_none());
        assert!(store.list_services().await.is_empty());
        assert!(store.list_all_tools().await.is_empty());
    }

    #[tokio::test]
    async fn db_source_queues_control_requests_for_mutation_variants() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-queue-variants".to_string()),
        })
        .unwrap();

        let global_name = store
            .add_service_for_agent("agent-a", "svc", stdio_config())
            .await
            .unwrap();
        assert_eq!(global_name, "svc_byagent_agent-a");

        let mut updated = stdio_config();
        updated.args = vec!["updated".to_string()];
        store
            .update_service("svc-update", updated.clone())
            .await
            .unwrap();
        store
            .patch_service("svc-patch", serde_json::json!({"description": "patched"}))
            .await
            .unwrap();
        store.remove_service("svc-remove").await.unwrap();
        store
            .assign_service_to_agent("agent-a", "svc-assign")
            .await
            .unwrap();
        store
            .unassign_service_from_agent("agent-a", "svc-unassign")
            .await
            .unwrap();
        store.connect_service("svc-connect").await.unwrap();
        store.disconnect_service("svc-disconnect").await.unwrap();
        store.restart_service("svc-restart").await.unwrap();
        store.reset_config().await.unwrap();

        let events = store
            .cache()
            .get_all_events_async(ONLYDB_CONTROL_EVENT_TYPE)
            .await
            .unwrap();
        assert_eq!(events.len(), 10);

        let queued_add = events
            .values()
            .find(|event| {
                event["type"] == serde_json::json!("ServiceAddRequested")
                    && event["payload"]["service_name"] == serde_json::json!("svc_byagent_agent-a")
            })
            .unwrap();
        assert_eq!(
            queued_add["payload"]["service_original_name"],
            serde_json::json!("svc")
        );
        assert_eq!(
            queued_add["payload"]["agent_id"],
            serde_json::json!("agent-a")
        );
        assert_eq!(queued_add["status"], serde_json::json!("pending"));

        let queued_update = events
            .values()
            .find(|event| {
                event["type"] == serde_json::json!("ServiceUpdateRequested")
                    && event["payload"]["service_name"] == serde_json::json!("svc-update")
            })
            .unwrap();
        assert_eq!(
            queued_update["payload"]["config"]["args"],
            serde_json::json!(["updated"])
        );

        let expected = vec![
            ("ServicePatchRequested", Some("svc-patch")),
            ("ServiceRemoveRequested", Some("svc-remove")),
            ("ServiceAssignRequested", Some("svc-assign")),
            ("ServiceUnassignRequested", Some("svc-unassign")),
            ("ServiceConnectRequested", Some("svc-connect")),
            ("ServiceDisconnectRequested", Some("svc-disconnect")),
            ("ServiceRestartRequested", Some("svc-restart")),
            ("StoreResetRequested", None),
        ];
        for (event_type, service_name) in expected {
            let event = events
                .values()
                .find(|event| {
                    if event["type"] != serde_json::json!(event_type) {
                        return false;
                    }
                    match service_name {
                        Some(name) => event["payload"]["service_name"] == serde_json::json!(name),
                        None => true,
                    }
                })
                .unwrap();
            assert_eq!(event["status"], serde_json::json!("pending"));
            assert_eq!(event["source"], serde_json::json!("onlydb"));
        }

        assert!(!std::path::Path::new(&path).exists());
        assert!(store
            .cache()
            .get_entity("services", "svc-update")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_relation("agent_services", "agent-a")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn db_source_runtime_use_does_not_write_shared_runtime_state() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-db-source-runtime-read-only".to_string()),
        })
        .unwrap();

        store
            .cache()
            .put_entity(
                "services",
                "svc",
                serde_json::to_value(ServiceEntity {
                    service_global_name: "svc".to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: GLOBAL_AGENT_STORE.to_string(),
                    config: serde_json::to_value(stdio_config()).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store.list_services().await;

        let cached_before = store.service_status_payload(
            "svc",
            HealthStatus::Disconnected,
            Some("main node state".to_string()),
            Vec::new(),
        );
        store
            .cache()
            .put_state(
                "service_status",
                "svc",
                serde_json::to_value(cached_before.clone()).unwrap(),
            )
            .await
            .unwrap();

        let healthy = store
            .record_health_check_result("svc", true, Some(1.0), None)
            .await
            .unwrap();
        assert_eq!(healthy.health_status, HealthStatus::Healthy);

        let failed = store
            .mark_service_retryable_failure("svc", "onlydb local failure".to_string())
            .await
            .unwrap();
        assert!(matches!(
            failed.health_status,
            HealthStatus::CircuitOpen | HealthStatus::Disconnected
        ));

        store
            .cache_service_connected(
                "svc",
                &[crate::registry::ToolInfo {
                    name: "echo".to_string(),
                    description: "echo".to_string(),
                    schema: serde_json::json!({"type": "object"}),
                }],
            )
            .await
            .unwrap();

        let cached_after: ServiceStatus = serde_json::from_value(
            store
                .cache()
                .get_state("service_status", "svc")
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(cached_after, cached_before);
        assert!(store
            .cache()
            .get_entity("tools", "svc_echo")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_relation("service_tools", "svc")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .cache()
            .get_all_events_async("service")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn local_source_processes_onlydb_control_requests() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("test-control-request-worker".to_string()),
        })
        .unwrap();
        store
            .cache()
            .put_event(
                ONLYDB_CONTROL_EVENT_TYPE,
                "evt-add",
                serde_json::json!({
                    "id": "evt-add",
                    "type": "ServiceAddRequested",
                    "payload": {
                        "service_name": "queued",
                        "service_original_name": "queued",
                        "agent_id": GLOBAL_AGENT_STORE,
                        "config": stdio_config(),
                    },
                    "source": "onlydb",
                    "created_at": 111,
                    "dedup_key": "ServiceAddRequested:global_agent_store:queued",
                    "trace_id": "evt-add",
                    "status": "pending",
                }),
            )
            .await
            .unwrap();

        let processed = store.process_control_requests().await.unwrap();
        assert_eq!(processed, 1);
        assert!(store
            .cache()
            .get_entity("services", "queued")
            .await
            .unwrap()
            .is_some());
        let event = store
            .cache()
            .get_event(ONLYDB_CONTROL_EVENT_TYPE, "evt-add")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event["status"], serde_json::json!("completed"));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn global_relation_keeps_multiple_services() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc-a", stdio_config()).await.unwrap();
        store.add_service("svc-b", stdio_config()).await.unwrap();

        let relation = store
            .cache()
            .get_relation("agent_services", "global_agent_store")
            .await
            .unwrap()
            .unwrap();
        let relation: AgentServiceRelation = serde_json::from_value(relation).unwrap();
        let names: Vec<String> = relation
            .services
            .into_iter()
            .map(|item| item.service_global_name)
            .collect();
        assert!(names.contains(&"svc-a".to_string()));
        assert!(names.contains(&"svc-b".to_string()));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn switch_cache_storage_migrates_runtime_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();
        store
            .assign_service_to_agent("agent-a", "svc")
            .await
            .unwrap();

        let snapshot = store
            .switch_cache_storage(CacheStorage::Memory, None, None)
            .await
            .unwrap();
        assert!(snapshot.entities["services"].contains_key("svc"));
        assert!(snapshot.relations["agent_services"].contains_key("global_agent_store"));
        assert!(snapshot.relations["agent_services"].contains_key("agent-a"));
        assert!(snapshot.states["service_status"].contains_key("svc"));

        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_some());
        let agent_services = store.list_agent_service_names("agent-a").await.unwrap();
        assert_eq!(agent_services, vec!["svc".to_string()]);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn switch_cache_storage_updates_namespace() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::Memory),
            redis_url: None,
            namespace: Some("before-switch".to_string()),
        })
        .unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let snapshot = store
            .switch_cache_storage(CacheStorage::Memory, None, Some("after-switch".to_string()))
            .await
            .unwrap();

        assert!(snapshot.entities["services"].contains_key("svc"));
        assert_eq!(store.namespace(), "after-switch");

        let inspect = store.cache_inspect().await.unwrap();
        assert_eq!(inspect["namespace"], serde_json::json!("after-switch"));
        let collections = inspect["collections"].as_array().unwrap();
        assert!(collections.iter().any(|value| {
            value
                .as_str()
                .map(|text| text.starts_with("after-switch:entity:services"))
                .unwrap_or(false)
        }));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn openkeyv_memory_backend_writes_cache_layers() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-openkeyv-memory".to_string()),
        })
        .unwrap();

        store.add_service("svc", stdio_config()).await.unwrap();

        assert_eq!(
            store.current_cache_storage().await,
            CacheStorage::OpenKeyvMemory
        );
        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_some());
        assert!(store
            .cache()
            .get_state("service_status", "svc")
            .await
            .unwrap()
            .is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn switch_cache_storage_to_openkeyv_memory_migrates_runtime_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup(Some(&path)).unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let snapshot = store
            .switch_cache_storage(CacheStorage::OpenKeyvMemory, None, None)
            .await
            .unwrap();

        assert!(snapshot.entities["services"].contains_key("svc"));
        assert_eq!(
            store.current_cache_storage().await,
            CacheStorage::OpenKeyvMemory
        );
        assert!(store
            .cache()
            .get_entity("services", "svc")
            .await
            .unwrap()
            .is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn update_and_patch_service_update_runtime_cache() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-update-patch".to_string()),
        })
        .unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let mut updated = stdio_config();
        updated.args = vec!["updated".to_string()];
        store.update_service("svc", updated).await.unwrap();
        let config = store.get_service_config("svc").await.unwrap().unwrap();
        assert_eq!(config["args"], serde_json::json!(["updated"]));

        store
            .patch_service("svc", serde_json::json!({"description": "patched"}))
            .await
            .unwrap();
        let config = store.get_service_config("svc").await.unwrap().unwrap();
        assert_eq!(config["description"], serde_json::json!("patched"));
    }

    #[tokio::test]
    async fn event_history_and_cache_health_are_reported() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-event-health".to_string()),
        })
        .unwrap();

        store
            .publish_event("TEST_EVENT", serde_json::json!({"ok": true}), true)
            .await
            .unwrap();
        let history = store.event_history(10).await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, "TEST_EVENT");

        store.add_service("svc", stdio_config()).await.unwrap();
        let health = store.cache_health_check().await.unwrap();
        assert_eq!(health["backend"], serde_json::json!("openkeyv_memory"));
        assert!(health["entities"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("services")));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn list_tools_uses_registry_without_transport_connection() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-list-tools-registry".to_string()),
        })
        .unwrap();
        store.add_service("svc", stdio_config()).await.unwrap();

        let tools = store.list_tools("svc").await.unwrap();
        assert!(tools.is_empty());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn connect_service_failure_opens_circuit_and_schedules_retry() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-connect-failure-status".to_string()),
        })
        .unwrap();
        store
            .add_service("broken", broken_stdio_config())
            .await
            .unwrap();

        let err = store
            .connect_service("broken")
            .await
            .unwrap_err()
            .to_string();
        let status = store
            .cached_service_status("broken")
            .await
            .unwrap()
            .unwrap();
        let service = store.find_service("broken").await.unwrap();

        assert!(err.contains("Connection failed"));
        assert_eq!(status.health_status, HealthStatus::CircuitOpen);
        assert_eq!(status.connection_attempts, 1);
        assert!(status.current_error.is_some());
        assert!(status.next_retry_time.is_some());
        assert_eq!(service.status, ConnectionStatus::Error);

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn automatic_retry_respects_backoff_and_enters_half_open_when_due() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-retry-backoff".to_string()),
        })
        .unwrap();
        store
            .add_service("broken", broken_stdio_config())
            .await
            .unwrap();
        store.connect_service("broken").await.unwrap_err();

        let blocked = store
            .connect_service_internal("broken", true)
            .await
            .unwrap_err()
            .to_string();
        assert!(blocked.contains("backoff active"));

        let mut due = store
            .cached_service_status("broken")
            .await
            .unwrap()
            .unwrap();
        due.health_status = HealthStatus::CircuitOpen;
        due.next_retry_time = Some(MCPStore::now_timestamp_f64() - 1.0);
        store.put_service_status_payload(&due).await.unwrap();

        let transitioned = store.health_check("broken").await.unwrap();
        assert_eq!(transitioned.health_status, HealthStatus::HalfOpen);
        assert!(transitioned.lease_deadline.is_some());

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn successful_health_check_clears_retry_state() {
        let path = temp_config_path();
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: Some(path.clone()),
            source_mode: SourceMode::Local,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-retry-reset".to_string()),
        })
        .unwrap();
        store
            .add_service("broken", broken_stdio_config())
            .await
            .unwrap();
        store.connect_service("broken").await.unwrap_err();

        let recovered = store
            .record_health_check_result("broken", true, Some(12.0), None)
            .await
            .unwrap();

        assert_eq!(recovered.health_status, HealthStatus::Healthy);
        assert_eq!(recovered.connection_attempts, 0);
        assert_eq!(recovered.current_error, None);
        assert_eq!(recovered.next_retry_time, None);
        assert_eq!(recovered.hard_deadline, None);
        assert_eq!(recovered.latency_p95, Some(12.0));
        assert_eq!(recovered.latency_p99, Some(12.0));

        std::fs::remove_file(path).ok();
    }

    #[tokio::test]
    async fn db_load_does_not_rewrite_cached_agent_relations() {
        let store = MCPStore::setup_with_options(StoreOptions {
            config_path: None,
            source_mode: SourceMode::Db,
            backend: Some(CacheStorage::OpenKeyvMemory),
            redis_url: None,
            namespace: Some("test-db-load-readonly".to_string()),
        })
        .unwrap();
        let service_name = "svc_byagent_agent-a";
        let config = stdio_config();
        store
            .cache()
            .put_entity(
                "services",
                service_name,
                serde_json::to_value(ServiceEntity {
                    service_global_name: service_name.to_string(),
                    service_original_name: "svc".to_string(),
                    source_agent: "agent-a".to_string(),
                    config: serde_json::to_value(config).unwrap(),
                    added_time: 111,
                })
                .unwrap(),
            )
            .await
            .unwrap();
        store
            .cache()
            .put_relation(
                "agent_services",
                "agent-a",
                serde_json::to_value(AgentServiceRelation {
                    services: vec![ServiceRelationItem {
                        service_original_name: "svc".to_string(),
                        service_global_name: service_name.to_string(),
                        client_id: service_name.to_string(),
                        established_time: 111,
                        last_access: Some(222),
                    }],
                })
                .unwrap(),
            )
            .await
            .unwrap();

        store.load_from_config().await.unwrap();

        let relation = store
            .cache()
            .get_relation("agent_services", "agent-a")
            .await
            .unwrap()
            .unwrap();
        let relation: AgentServiceRelation = serde_json::from_value(relation).unwrap();
        assert_eq!(relation.services[0].established_time, 111);
        assert_eq!(relation.services[0].last_access, Some(222));
    }
}
