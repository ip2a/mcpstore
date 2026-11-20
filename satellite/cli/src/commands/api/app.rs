use std::{path::Path as FsPath, sync::Arc};

use axum::{extract::State, Json};
use mcpstore::{
    config::{ConfigError, HostsConfig},
    AppConfig,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    envelope::{success, ApiError, ApiResult},
    panel_settings::{self, PanelSettings},
    ApiState, PanelCtx,
};

#[derive(Deserialize)]
pub(super) struct UpdateSettingsRequest {
    language: Option<String>,
    diagnostics: Option<UpdateDiagnosticsRequest>,
    api: Option<UpdateApiRequest>,
    web: Option<UpdateWebRequest>,
    gateway: Option<UpdateGatewayRequest>,
    hosts: Option<HostsConfig>,
    service_tabs: Option<UpdateServiceTabsRequest>,
}

#[derive(Deserialize)]
struct UpdateServiceTabsRequest {
    sort: Option<String>,
    show_disconnected: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateApiRequest {
    port: Option<u16>,
}

#[derive(Deserialize)]
struct UpdateWebRequest {
    port: Option<u16>,
    auth: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateGatewayRequest {
    port: Option<u16>,
}

fn deserialize_nullable_u64<'de, D>(deserializer: D) -> Result<Option<Option<u64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<u64>::deserialize(deserializer)?))
}

#[derive(Deserialize)]
struct UpdateDiagnosticsRequest {
    runtime_log: Option<UpdateRuntimeLogRequest>,
    requests: Option<UpdateRequestsRequest>,
    gateway_calls: Option<UpdateGatewayCallsRequest>,
    frames: Option<UpdateFramesRequest>,
    event_history: Option<UpdateEventHistoryRequest>,
    stdio_stderr: Option<UpdateStdioStderrRequest>,
    daemon_output: Option<UpdateDaemonOutputRequest>,
}

#[derive(Deserialize)]
struct UpdateRuntimeLogRequest {
    enabled: Option<bool>,
    level: Option<String>,
    max_size_bytes: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    retention_days: Option<Option<u64>>,
}

#[derive(Deserialize)]
struct UpdateRequestsRequest {
    enabled: Option<bool>,
    capture_tool: Option<bool>,
    capture_management: Option<bool>,
    capture_cli: Option<bool>,
    capture_payload: Option<bool>,
    max_payload_bytes: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    retention_days: Option<Option<u64>>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    max_records: Option<Option<u64>>,
}

#[derive(Deserialize)]
struct UpdateGatewayCallsRequest {
    enabled: Option<bool>,
    persist: Option<bool>,
    memory_capacity: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    retention_days: Option<Option<u64>>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    max_calls: Option<Option<u64>>,
}

#[derive(Deserialize)]
struct UpdateFramesRequest {
    enabled: Option<bool>,
    capture_gateway: Option<bool>,
    capture_upstream: Option<bool>,
    capture_payload: Option<bool>,
    max_payload_bytes: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    retention_days: Option<Option<u64>>,
    #[serde(default, deserialize_with = "deserialize_nullable_u64")]
    max_records: Option<Option<u64>>,
}

#[derive(Deserialize)]
struct UpdateEventHistoryRequest {
    enabled: Option<bool>,
    capacity: Option<usize>,
}

#[derive(Deserialize)]
struct UpdateStdioStderrRequest {
    enabled: Option<bool>,
    tail_lines: Option<usize>,
    forward_to_runtime_log: Option<bool>,
    include_in_error: Option<bool>,
}

#[derive(Deserialize)]
struct UpdateDaemonOutputRequest {
    enabled: Option<bool>,
}

pub(super) async fn health<T: PanelCtx>(State(state): State<Arc<T>>) -> Json<Value> {
    let store = match state.daemon() {
        Some(daemon) => Some(daemon.current_store_name().await),
        None => None,
    };
    Json(json!({
        "status": "ok",
        "store": store,
    }))
}

pub(super) async fn daemon_meta(State(state): State<Arc<ApiState>>) -> ApiResult {
    Ok(success(
        "Daemon metadata fetched",
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "node": {
                "id": state.store.panel_id(),
                "mode": match state.store.panel_role() {
                    mcpstore::PanelRole::ControlPanel => "control",
                    mcpstore::PanelRole::DataPanel { .. } => "data",
                },
            },
            "features": {
                "app_host_routing_v1": true
            },
        }),
    ))
}

pub(super) async fn meta<T: PanelCtx>(State(state): State<Arc<T>>) -> ApiResult {
    let payload = app_meta_payload(state.as_ref())?;
    Ok(success("App metadata fetched successfully", payload))
}

pub(super) async fn update_settings<T: PanelCtx>(
    State(state): State<Arc<T>>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> ApiResult {
    let config_manager = state.config_manager();
    let mut config = config_manager
        .load_app_config_or_default()
        .map_err(config_api_error)?;

    // Validate every input before any file write lands.
    let language = match payload.language {
        Some(language) => Some(
            panel_settings::normalize_language(&language)
                .map_err(|message| ApiError::invalid_parameter(message, Some("language")))?,
        ),
        None => None,
    };

    if let Some(api) = payload.api {
        if let Some(port) = api.port {
            if port == 0 {
                return Err(ApiError::invalid_parameter(
                    "API port must be greater than 0",
                    Some("api.port"),
                ));
            }
            config.api.port = port;
        }
    }

    if let Some(web) = payload.web {
        if let Some(port) = web.port {
            if port == 0 {
                return Err(ApiError::invalid_parameter(
                    "Web port must be greater than 0",
                    Some("web.port"),
                ));
            }
            config.web.port = port;
        }
        if let Some(auth) = web.auth {
            config.web.auth = auth;
        }
    }

    if let Some(gateway) = payload.gateway {
        if let Some(port) = gateway.port {
            if port == 0 {
                return Err(ApiError::invalid_parameter(
                    "Gateway port must be greater than 0",
                    Some("gateway.port"),
                ));
            }
            config.gateway.port = port;
        }
    }

    if let Some(diagnostics) = payload.diagnostics {
        if let Some(runtime_log) = diagnostics.runtime_log {
            if let Some(enabled) = runtime_log.enabled {
                config.diagnostics.runtime_log.enabled = enabled;
            }
            if let Some(level) = runtime_log.level {
                config.diagnostics.runtime_log.level = level;
            }
            if let Some(max_size_bytes) = runtime_log.max_size_bytes {
                config.diagnostics.runtime_log.max_size_bytes = max_size_bytes;
            }
            if let Some(retention_days) = runtime_log.retention_days {
                config.diagnostics.runtime_log.retention_days = retention_days;
            }
        }
        if let Some(requests) = diagnostics.requests {
            if let Some(enabled) = requests.enabled {
                config.diagnostics.requests.enabled = enabled;
            }
            if let Some(value) = requests.capture_tool {
                config.diagnostics.requests.capture_tool = value;
            }
            if let Some(value) = requests.capture_management {
                config.diagnostics.requests.capture_management = value;
            }
            if let Some(value) = requests.capture_cli {
                config.diagnostics.requests.capture_cli = value;
            }
            if let Some(value) = requests.capture_payload {
                config.diagnostics.requests.capture_payload = value;
            }
            if let Some(value) = requests.max_payload_bytes {
                config.diagnostics.requests.max_payload_bytes = value;
            }
            if let Some(value) = requests.retention_days {
                config.diagnostics.requests.retention_days = value;
            }
            if let Some(value) = requests.max_records {
                config.diagnostics.requests.max_records = value;
            }
        }
        if let Some(calls) = diagnostics.gateway_calls {
            if let Some(value) = calls.enabled {
                config.diagnostics.gateway_calls.enabled = value;
            }
            if let Some(value) = calls.persist {
                config.diagnostics.gateway_calls.persist = value;
            }
            if let Some(value) = calls.memory_capacity {
                config.diagnostics.gateway_calls.memory_capacity = value;
            }
            if let Some(value) = calls.retention_days {
                config.diagnostics.gateway_calls.retention_days = value;
            }
            if let Some(value) = calls.max_calls {
                config.diagnostics.gateway_calls.max_calls = value;
            }
        }
        if let Some(frames) = diagnostics.frames {
            if let Some(value) = frames.enabled {
                config.diagnostics.frames.enabled = value;
            }
            if let Some(value) = frames.capture_gateway {
                config.diagnostics.frames.capture_gateway = value;
            }
            if let Some(value) = frames.capture_upstream {
                config.diagnostics.frames.capture_upstream = value;
            }
            if let Some(value) = frames.capture_payload {
                config.diagnostics.frames.capture_payload = value;
            }
            if let Some(value) = frames.max_payload_bytes {
                config.diagnostics.frames.max_payload_bytes = value;
            }
            if let Some(value) = frames.retention_days {
                config.diagnostics.frames.retention_days = value;
            }
            if let Some(value) = frames.max_records {
                config.diagnostics.frames.max_records = value;
            }
        }
        if let Some(history) = diagnostics.event_history {
            if let Some(value) = history.enabled {
                config.diagnostics.event_history.enabled = value;
            }
            if let Some(value) = history.capacity {
                config.diagnostics.event_history.capacity = value;
            }
        }
        if let Some(stderr) = diagnostics.stdio_stderr {
            if let Some(value) = stderr.enabled {
                config.diagnostics.stdio_stderr.enabled = value;
            }
            if let Some(value) = stderr.tail_lines {
                config.diagnostics.stdio_stderr.tail_lines = value;
            }
            if let Some(value) = stderr.forward_to_runtime_log {
                config.diagnostics.stdio_stderr.forward_to_runtime_log = value;
            }
            if let Some(value) = stderr.include_in_error {
                config.diagnostics.stdio_stderr.include_in_error = value;
            }
        }
        if let Some(output) = diagnostics.daemon_output {
            if let Some(value) = output.enabled {
                config.diagnostics.daemon_output.enabled = value;
            }
        }
    }

    // Validate the complete candidate before panel loading can migrate or write anything.
    mcpstore::config::validate_app_config(&config).map_err(config_api_error)?;

    // One in-process lock covers panel-settings read, mutate and persist; the
    // two file writes are atomic individually but are not one transaction.
    let _persistence = panel_settings::persistence_lock();
    let mut panel_settings =
        panel_settings::load_or_migrate(config_manager).map_err(ApiError::invalid_request)?;
    if let Some(language) = language {
        panel_settings.language = language;
    }
    if let Some(hosts) = payload.hosts {
        panel_settings.hosts = hosts;
    }
    if let Some(service_tabs) = payload.service_tabs {
        if let Some(sort) = service_tabs.sort {
            panel_settings.service_tabs.sort = panel_settings::normalize_service_tab_sort(&sort)
                .map_err(|message| {
                    ApiError::invalid_parameter(message, Some("service_tabs.sort"))
                })?;
        }
        if let Some(show_disconnected) = service_tabs.show_disconnected {
            panel_settings.service_tabs.show_disconnected = show_disconnected;
        }
    }

    config_manager
        .save_app_config(&config)
        .map_err(config_api_error)?;
    panel_settings::save_for_manager(config_manager, &panel_settings)
        .map_err(ApiError::invalid_request)?;

    Ok(success(
        "Settings saved successfully",
        settings_payload(
            &config,
            &panel_settings,
            &super::panel_auth::PanelAuthGuard::token_path(config_manager),
        ),
    ))
}

fn app_meta_payload<T: PanelCtx>(state: &T) -> Result<Value, ApiError> {
    let config_manager = state.config_manager();
    let config = config_manager
        .load_app_config_or_default()
        .map_err(config_api_error)?;
    let config_path = config_manager.app_config_path();
    let config_content = if config_path.exists() {
        std::fs::read_to_string(config_path).map_err(config_io_api_error)?
    } else {
        config_manager
            .default_app_config_toml()
            .map_err(config_api_error)?
    };

    let _persistence = panel_settings::persistence_lock();
    let panel_settings =
        panel_settings::load_or_migrate(config_manager).map_err(ApiError::invalid_request)?;

    let (node_id, node_mode) = match state.daemon() {
        Some(daemon) => (
            daemon.panel_id(),
            match daemon.panel_role() {
                mcpstore::PanelRole::ControlPanel => "control",
                mcpstore::PanelRole::DataPanel { .. } => "data",
            },
        ),
        // The standalone web panel is a control panel without a store.
        None => (mcpstore::state::CONTROL_NODE_ID.to_string(), "control"),
    };

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "node": {
            "id": node_id,
            "mode": node_mode,
        },
        "settings": settings_payload(
            &config,
            &panel_settings,
            &super::panel_auth::PanelAuthGuard::token_path(config_manager),
        ),
        "settings_paths": settings_paths_payload(config_manager.mcp_path()),
        "config_file": {
            "path": config_path.display().to_string(),
            "format": "toml",
            "content": config_content,
        },
    }))
}

fn settings_payload(
    config: &AppConfig,
    panel_settings: &PanelSettings,
    web_auth_token_path: &std::path::Path,
) -> Value {
    let diagnostics = serde_json::to_value(&config.diagnostics)
        .expect("diagnostics configuration is always serializable");
    json!({
        "language": panel_settings.language,
        "api": {
            "host": config.api.host,
            "port": config.api.port,
            "url_prefix": config.api.url_prefix,
        },
        "web": {
            "host": config.web.host,
            "port": config.web.port,
            "auth": config.web.auth,
            "auth_token_path": web_auth_token_path.display().to_string(),
        },
        "gateway": {
            "port": config.gateway.port,
        },
        "diagnostics": diagnostics,
        "hosts": hosts_payload(&panel_settings.hosts),
        "service_tabs": {
            "sort": panel_settings.service_tabs.sort,
            "show_disconnected": panel_settings.service_tabs.show_disconnected,
        },
    })
}

fn hosts_payload(hosts: &HostsConfig) -> Value {
    let mut value = serde_json::Map::new();
    value.insert("active".to_string(), json!(hosts.active));
    for (name, entry) in &hosts.entries {
        value.insert(name.clone(), json!({ "urls": entry.urls }));
    }
    Value::Object(value)
}

fn settings_paths_payload(mcp_path: &FsPath) -> Value {
    let base = mcp_path.parent().unwrap_or_else(|| FsPath::new("."));
    let log_dir = base.join("logs");
    let log_file_name = "daemon.log";

    json!({
        "log_dir": log_dir.display().to_string(),
        "log_file_name": log_file_name,
        "log_file_path": log_dir.join(log_file_name).display().to_string(),
    })
}

fn config_api_error(error: ConfigError) -> ApiError {
    ApiError::invalid_request(error.to_string())
}

pub(super) fn config_io_api_error(error: std::io::Error) -> ApiError {
    ApiError::invalid_request(error.to_string())
}
