use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::commands::api::{envelope::{success, ApiError, ApiResult}, ApiState};

#[derive(Deserialize)]
pub(super) struct ExportQuery {
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "toml".to_string()
}

/// GET /api/config/export?format=toml|json
/// Export current scope configuration from openkeyv
pub(super) async fn config_export(
    Query(query): Query<ExportQuery>,
    State(_state): State<Arc<ApiState>>,
) -> ApiResult {
    let config = mcpstore::ScopesConfig::load_or_default();

    let data = match query.format.as_str() {
        "json" => serde_json::to_value(&config)
            .map_err(|e| ApiError::internal(e.to_string()))?,
        _ => {
            let toml_str = toml::to_string_pretty(&config)
                .map_err(|e| ApiError::internal(e.to_string()))?;
            serde_json::json!({ "content": toml_str, "format": "toml" })
        }
    };

    Ok(success("Configuration exported", data))
}

/// POST /api/config/reload
/// Reload scopes.toml from disk and apply to openkeyv
pub(super) async fn config_reload(State(state): State<Arc<ApiState>>) -> ApiResult {
    state
        .store
        .load_scopes_config()
        .await
        .map_err(ApiError::from_store)?;

    Ok(success(
        "Scopes configuration reloaded",
        serde_json::json!({"status": "ok"}),
    ))
}

#[derive(Deserialize)]
pub(super) struct ImportBody {
    config: String,
    #[serde(default = "default_format")]
    format: String,
}

/// POST /api/config/import
/// Import scope configuration, write to scopes.toml and apply to openkeyv
pub(super) async fn config_import(
    State(state): State<Arc<ApiState>>,
    Json(body): Json<ImportBody>,
) -> ApiResult {
    let config: mcpstore::ScopesConfig = match body.format.as_str() {
        "json" => serde_json::from_str(&body.config)
            .map_err(|e| ApiError::invalid_request(format!("Invalid JSON: {}", e)))?,
        _ => toml::from_str(&body.config)
            .map_err(|e| ApiError::invalid_request(format!("Invalid TOML: {}", e)))?,
    };

    // Write to file
    let path = mcpstore::ScopesConfig::default_path()
        .ok_or_else(|| ApiError::internal("Cannot determine home directory"))?;

    let content = toml::to_string_pretty(&config)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    std::fs::write(&path, content)
        .map_err(|e| ApiError::internal(format!("Failed to write config: {}", e)))?;

    // Reload from file
    state
        .store
        .load_scopes_config()
        .await
        .map_err(ApiError::from_store)?;

    Ok(success(
        "Configuration imported and applied",
        serde_json::json!({
            "status": "ok",
            "path": path.display().to_string()
        }),
    ))
}
