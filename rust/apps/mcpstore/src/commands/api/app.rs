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
    ApiState,
};

#[derive(Deserialize)]
pub(super) struct UpdateSettingsRequest {
    language: Option<String>,
    diagnostics: Option<UpdateDiagnosticsRequest>,
    api: Option<UpdateApiRequest>,
    web: Option<UpdateWebRequest>,
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

pub(super) async fn health(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "store": state.store.current_store_name().await,
    }))
}

pub(super) async fn meta(State(state): State<Arc<ApiState>>) -> ApiResult {
    let payload = app_meta_payload(&state)?;
    Ok(success("应用元信息获取成功", payload))
}

pub(super) async fn update_settings(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> ApiResult {
    let config_manager = state.store.config_manager();
    let mut config = config_manager
        .load_app_config_or_default()
        .map_err(config_api_error)?;

    if let Some(language) = payload.language {
        config.ui.language = normalize_ui_language(&language)?;
    }

    if let Some(api) = payload.api {
        if let Some(port) = api.port {
            if port == 0 {
                return Err(ApiError::invalid_parameter(
                    "后端端口必须大于 0",
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
                    "前端端口必须大于 0",
                    Some("web.port"),
                ));
            }
            config.web.port = port;
        }
    }

    if let Some(hosts) = payload.hosts {
        config.hosts = hosts;
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
                        "运行Logs大小上限必须大于 0",
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

    config_manager
        .save_app_config(&config)
        .map_err(config_api_error)?;

    Ok(success("设置保存成功", settings_payload(&config)))
}

fn app_meta_payload(state: &ApiState) -> Result<Value, ApiError> {
    let config_manager = state.store.config_manager();
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

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "settings": settings_payload(&config),
        "settings_paths": settings_paths_payload(config_manager.mcp_path()),
        "config_file": {
            "path": config_path.display().to_string(),
            "format": "toml",
            "content": config_content,
        },
    }))
}

fn settings_payload(config: &AppConfig) -> Value {
    json!({
        "language": api_ui_language(&config.ui.language),
        "api": {
            "host": config.api.host,
            "port": config.api.port,
            "url_prefix": config.api.url_prefix,
        },
        "web": {
            "host": config.web.host,
            "port": config.web.port,
        },
        "diagnostics": {
            "enabled": config.diagnostics.enabled,
            "runtime_log": {
                "enabled": config.diagnostics.runtime_log.enabled,
                "max_size_bytes": config.diagnostics.runtime_log.max_size_bytes,
                "retention_days": config.diagnostics.runtime_log.retention_days,
            },
        },
        "hosts": hosts_payload(&config.hosts),
    })
}

fn hosts_payload(hosts: &HostsConfig) -> Value {
    let mut value = serde_json::Map::new();
    value.insert("active".to_string(), json!(hosts.active));
    for (name, entry) in &hosts.entries {
        value.insert(name.clone(), json!({ "url": entry.url }));
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

fn normalize_ui_language(language: &str) -> Result<String, ApiError> {
    match language.trim() {
        "auto" => Ok("auto".to_string()),
        "zh" | "zh-cn" => Ok("zh".to_string()),
        "en" => Ok("en".to_string()),
        _ => Err(ApiError::invalid_parameter(
            "语言必须是 auto、zh 或 en",
            Some("language"),
        )),
    }
}

fn api_ui_language(language: &str) -> &str {
    match language {
        "zh-cn" => "zh",
        value => value,
    }
}

fn config_api_error(error: ConfigError) -> ApiError {
    ApiError::invalid_request(error.to_string())
}

pub(super) fn config_io_api_error(error: std::io::Error) -> ApiError {
    ApiError::invalid_request(error.to_string())
}
