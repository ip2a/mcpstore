use std::{path::Path as FsPath, sync::Arc};

use axum::{extract::State, Json};
use mcpstore::{
    config::{ConfigError, HistoryPayload, HistoryStorage},
    AppConfig,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    envelope::{success, ApiError, ApiResult},
    parse::cache_storage_label,
    ApiState,
};

#[derive(Deserialize)]
pub(super) struct UpdateSettingsRequest {
    language: Option<String>,
    default_backup_dir: Option<String>,
    logging: Option<UpdateLoggingRequest>,
    diagnostics: Option<UpdateDiagnosticsRequest>,
}

#[derive(Deserialize)]
struct UpdateLoggingRequest {
    max_size_bytes: Option<u64>,
    retention_days: Option<Option<u64>>,
}

#[derive(Deserialize)]
struct UpdateDiagnosticsRequest {
    enabled: Option<bool>,
    runtime_log: Option<UpdateRuntimeLogRequest>,
    history: Option<UpdateHistoryRequest>,
}

#[derive(Deserialize)]
struct UpdateRuntimeLogRequest {
    enabled: Option<bool>,
    max_size_bytes: Option<u64>,
}

#[derive(Deserialize)]
struct UpdateHistoryRequest {
    enabled: Option<bool>,
    storage: Option<HistoryStorage>,
    max_records: Option<usize>,
    max_size_bytes: Option<u64>,
    retention_days: Option<Option<u64>>,
    payload: Option<HistoryPayload>,
}

pub(super) async fn health(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "backend": cache_storage_label(state.store.current_cache_storage().await),
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

    if let Some(default_backup_dir) = payload.default_backup_dir {
        let value = default_backup_dir.trim();
        if value.is_empty() {
            return Err(ApiError::invalid_parameter(
                "默认备份目录不能为空",
                Some("default_backup_dir"),
            ));
        }
        config.ui.default_backup_dir = value.to_string();
    }

    if let Some(logging) = payload.logging {
        if let Some(max_size_bytes) = logging.max_size_bytes {
            if max_size_bytes == 0 {
                return Err(ApiError::invalid_parameter(
                    "日志大小上限必须大于 0",
                    Some("logging.max_size_bytes"),
                ));
            }
            config.ui.logging.max_size_bytes = max_size_bytes;
        }
        if let Some(retention_days) = logging.retention_days {
            config.ui.logging.retention_days = retention_days;
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
                        "运行日志大小上限必须大于 0",
                        Some("diagnostics.runtime_log.max_size_bytes"),
                    ));
                }
                config.diagnostics.runtime_log.max_size_bytes = max_size_bytes;
            }
        }
        if let Some(history) = diagnostics.history {
            if let Some(enabled) = history.enabled {
                config.diagnostics.history.enabled = enabled && config.diagnostics.enabled;
            }
            if let Some(storage) = history.storage {
                config.diagnostics.history.storage = storage;
            }
            if let Some(max_records) = history.max_records {
                if max_records == 0 {
                    return Err(ApiError::invalid_parameter(
                        "调用历史条数上限必须大于 0",
                        Some("diagnostics.history.max_records"),
                    ));
                }
                config.diagnostics.history.max_records = max_records;
            }
            if let Some(max_size_bytes) = history.max_size_bytes {
                if max_size_bytes == 0 {
                    return Err(ApiError::invalid_parameter(
                        "调用历史大小上限必须大于 0",
                        Some("diagnostics.history.max_size_bytes"),
                    ));
                }
                config.diagnostics.history.max_size_bytes = max_size_bytes;
            }
            if let Some(retention_days) = history.retention_days {
                config.diagnostics.history.retention_days = retention_days;
            }
            if let Some(payload) = history.payload {
                config.diagnostics.history.payload = payload;
            }
        }
    }

    config_manager
        .save_app_config(&config)
        .map_err(config_api_error)?;
    state
        .store
        .update_history_config(mcpstore::config::HistoryConfig {
            enabled: config.diagnostics.enabled && config.diagnostics.history.enabled,
            ..config.diagnostics.history.clone()
        })
        .await;

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
        "settings_paths": settings_paths_payload(config_manager.mcp_path(), &config),
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
        "default_backup_dir": config.ui.default_backup_dir,
        "logging": {
            "max_size_bytes": config.ui.logging.max_size_bytes,
            "retention_days": config.ui.logging.retention_days,
        },
        "diagnostics": {
            "enabled": config.diagnostics.enabled,
            "runtime_log": {
                "enabled": config.diagnostics.runtime_log.enabled,
                "max_size_bytes": config.diagnostics.runtime_log.max_size_bytes,
            },
            "history": {
                "enabled": config.diagnostics.history.enabled,
                "storage": config.diagnostics.history.storage,
                "max_records": config.diagnostics.history.max_records,
                "max_size_bytes": config.diagnostics.history.max_size_bytes,
                "retention_days": config.diagnostics.history.retention_days,
                "payload": config.diagnostics.history.payload,
            }
        },
    })
}

fn settings_paths_payload(mcp_path: &FsPath, config: &AppConfig) -> Value {
    let base = mcp_path.parent().unwrap_or_else(|| FsPath::new("."));
    let backup_dir = FsPath::new(&config.ui.default_backup_dir);
    let backup_dir_resolved = if backup_dir.is_absolute() {
        backup_dir.to_path_buf()
    } else {
        base.join(backup_dir)
    };
    let log_dir = base.join("logs");
    let log_file_name = "mcpstore.log";

    json!({
        "backup_dir_base": base.display().to_string(),
        "backup_dir_input": config.ui.default_backup_dir,
        "backup_dir_resolved": backup_dir_resolved.display().to_string(),
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
