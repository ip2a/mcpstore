//! Plugin management over HTTP: registry status, market listing, manual
//! detection, install/uninstall, enable/disable. Uninstall and disable refuse
//! while the client still has gateway entries — those files are owned by the
//! wire ledger, and mutating a wired client behind its back corrupts it.

use std::sync::Arc;

use super::envelope::{success, ApiError, ApiResult};
use super::PanelCtx;
use axum::extract::{Path, State};
use serde_json::{json, Value};

fn market_urls<T: PanelCtx>(state: &T) -> Vec<String> {
    state
        .config_manager()
        .load_app_config_or_default()
        .map(|config| config.plugins.market_urls)
        .unwrap_or_default()
}

/// Registry view plus the trace cache: what is installed, what is enabled,
/// what detection saw, and the last market versions we know about.
pub(super) async fn plugins_list<T: PanelCtx>(State(_state): State<Arc<T>>) -> ApiResult {
    let trace = mcpstore::agents::registry::load_trace();
    let plugins: Vec<Value> = mcpstore::agents::registry::registry_report()
        .into_iter()
        .map(|report| {
            let market_version = trace.market_versions.get(&report.id);
            json!({
                "id": report.id,
                "label": report.label,
                "plugin_version": report.plugin_version,
                "spec_version": report.spec_version,
                "enabled": report.enabled,
                "error": report.error,
                "market_version": market_version,
            })
        })
        .collect();
    Ok(success(
        "插件清单获取成功",
        json!({
            "plugins": plugins,
            "last_detect": trace.last_detect,
            "market_fetched_at": trace.market_fetched_at,
        }),
    ))
}

/// The market index, refreshed into the trace cache, with install/update
/// status per entry.
pub(super) async fn plugins_market<T: PanelCtx>(State(state): State<Arc<T>>) -> ApiResult {
    let urls = market_urls(state.as_ref());
    let listings = mcpstore::agents::market::list_market(&urls)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        "插件市场获取成功",
        json!({
            "plugins": listings,
            "market_fetched_at": mcpstore::agents::registry::load_trace().market_fetched_at
        }),
    ))
}

/// Detect uninstalled clients on this machine using the market's discovery
/// rules. Reports only; nothing is installed.
pub(super) async fn plugins_detect<T: PanelCtx>(State(state): State<Arc<T>>) -> ApiResult {
    let urls = market_urls(state.as_ref());
    let hits = mcpstore::agents::market::detect_from_market(&urls)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success("客户端探测完成", json!({ "detected": hits })))
}

pub(super) async fn plugin_install<T: PanelCtx>(
    State(state): State<Arc<T>>,
    Path(id): Path<String>,
) -> ApiResult {
    let urls = market_urls(state.as_ref());
    mcpstore::agents::market::install(&id, &urls)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(
        &format!("插件 {id} 已安装（默认未启用）"),
        json!({ "id": id, "enabled": false }),
    ))
}

pub(super) async fn plugin_uninstall<T: PanelCtx>(
    State(_state): State<Arc<T>>,
    Path(id): Path<String>,
) -> ApiResult {
    mcpstore::agents::market::uninstall(&id)
        .await
        .map_err(ApiError::from_store)?;
    Ok(success(&format!("插件 {id} 已卸载"), json!({ "id": id })))
}

pub(super) async fn plugin_enable<T: PanelCtx>(
    State(_state): State<Arc<T>>,
    Path(id): Path<String>,
) -> ApiResult {
    mcpstore::agents::registry::enable(&id).map_err(ApiError::from_store)?;
    Ok(success(
        &format!("插件 {id} 已启用"),
        json!({ "id": id, "enabled": true }),
    ))
}

pub(super) async fn plugin_disable<T: PanelCtx>(
    State(_state): State<Arc<T>>,
    Path(id): Path<String>,
) -> ApiResult {
    mcpstore::agents::registry::disable(&id).map_err(ApiError::from_store)?;
    Ok(success(
        &format!("插件 {id} 已禁用"),
        json!({ "id": id, "enabled": false }),
    ))
}
