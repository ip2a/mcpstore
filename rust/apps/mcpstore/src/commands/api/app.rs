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
}

#[derive(Deserialize)]
struct UpdateApiRequest {
    port: Option<u16>,
}

#[derive(Deserialize)]
struct UpdateWebRequest {
    port: Option<u16>,
}

#[derive(Deserialize)]
struct UpdateGatewayRequest {
    port: Option<u16>,
}

#[derive(Deserialize)]
struct UpdateDiagnosticsRequest {
    enabled: Option<bool>,
    runtime_log: Option<UpdateRuntimeLogRequest>,
}

#[derive(Deserialize)]
struct UpdateRuntimeLogRequest {
    enabled: Option<bool>,
    max_size_bytes: Option<u64>,
    retention_days: Option<Option<u64>>,
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
        if let Some(enabled) = diagnostics.enabled {
            config.diagnostics.enabled = enabled;
        }
        if let Some(runtime_log) = diagnostics.runtime_log {
            if let Some(enabled) = runtime_log.enabled {
                config.diagnostics.runtime_log.enabled = enabled;
            }
            if let Some(max_size_bytes) = runtime_log.max_size_bytes {
                if max_size_bytes == 0 {
                    return Err(ApiError::invalid_parameter(
                        "Runtime log size limit must be greater than 0",
                        Some("diagnostics.runtime_log.max_size_bytes"),
                    ));
                }
                config.diagnostics.runtime_log.max_size_bytes = max_size_bytes;
            }
            if let Some(retention_days) = runtime_log.retention_days {
                config.diagnostics.runtime_log.retention_days = retention_days;
            }
        }
    }

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

    config_manager
        .save_app_config(&config)
        .map_err(config_api_error)?;
    panel_settings::save_for_manager(config_manager, &panel_settings)
        .map_err(ApiError::invalid_request)?;

    Ok(success(
        "Settings saved successfully",
        settings_payload(&config, &panel_settings),
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
        "settings": settings_payload(&config, &panel_settings),
        "settings_paths": settings_paths_payload(config_manager.mcp_path()),
        "config_file": {
            "path": config_path.display().to_string(),
            "format": "toml",
            "content": config_content,
        },
    }))
}

fn settings_payload(config: &AppConfig, panel_settings: &PanelSettings) -> Value {
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
        },
        "gateway": {
            "port": config.gateway.port,
        },
        "diagnostics": {
            "enabled": config.diagnostics.enabled,
            "runtime_log": {
                "enabled": config.diagnostics.runtime_log.enabled,
                "max_size_bytes": config.diagnostics.runtime_log.max_size_bytes,
                "retention_days": config.diagnostics.runtime_log.retention_days,
            },
        },
        "hosts": hosts_payload(&panel_settings.hosts),
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
    let log_file_name = "mcpstore.log";

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
