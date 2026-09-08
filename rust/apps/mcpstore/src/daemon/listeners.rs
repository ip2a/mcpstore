use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};

use axum::routing::any;
use axum::{http::StatusCode, Router};
use mcpstore::error::{Error, FailureCode};
use mcpstore::{AppConfig, ScopeRef};
use serde::Serialize;

use crate::commands::api::{self, ApiState};

/// daemon 托管的四个 HTTP 面。Kernel RPC（unix socket）不在此列，由 server.rs 持有。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerKey {
    Core,
    App,
    Web,
    Aggregate,
}

const ALL_KEYS: [ListenerKey; 4] = [
    ListenerKey::Core,
    ListenerKey::App,
    ListenerKey::Web,
    ListenerKey::Aggregate,
];

impl ListenerKey {
    fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::App => "app",
            Self::Web => "web",
            Self::Aggregate => "aggregate",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Core => 0,
            Self::App => 1,
            Self::Web => 2,
            Self::Aggregate => 3,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListenerStatus {
    pub key: &'static str,
    pub running: bool,
    pub bind: Option<SocketAddr>,
}

struct ListenerSlot {
    task: tokio::task::JoinHandle<()>,
    bind: SocketAddr,
}

/// listener 生命周期唯一主人：启停、热重绑、快照。
/// 热重绑语义：先 bind 新地址成功 → abort 旧任务；bind 失败时旧 listener 原样运行。
pub struct ListenerManager {
    // ponytail: 全局一把锁；daemon 管理操作串行到达，无争用场景
    slots: Mutex<[Option<ListenerSlot>; 4]>,
}

impl ListenerManager {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new([None, None, None, None]),
        }
    }

    /// daemon 启动：按 config.toml 拉起所有启用的面。任一面失败即停掉已启动的并返回错误。
    pub async fn start_all(&self, config: &AppConfig, state: &Arc<ApiState>) -> Result<(), Error> {
        let server = &config.server;
        let aggregate = &config.mcp_aggregate;
        for (key, port) in [
            (ListenerKey::Core, server.core_enabled.then_some(server.port)),
            (ListenerKey::App, server.app_enabled.then_some(server.app_port)),
            (ListenerKey::Web, server.web_enabled.then_some(server.web_port)),
            (
                ListenerKey::Aggregate,
                (aggregate.enabled && aggregate.transport == "streamable-http")
                    .then_some(aggregate.port),
            ),
        ] {
            match port {
                Some(port) => {
                    if let Err(error) = self.apply(key, resolve_bind(&server.host, port)?, state).await {
                        self.shutdown_all();
                        return Err(error);
                    }
                }
                None => tracing::info!("[DAEMON] {} face disabled", key.name()),
            }
        }
        Ok(())
    }

    /// 启用/重绑一个面。目标与现状相同则跳过（幂等）；bind 失败保留旧 listener，不会出现空窗。
    pub async fn apply(
        &self,
        key: ListenerKey,
        bind: SocketAddr,
        state: &Arc<ApiState>,
    ) -> Result<(), Error> {
        {
            let slots = self.slots.lock().expect("listener slots poisoned");
            if slots[key.index()]
                .as_ref()
                .is_some_and(|current| current.bind == bind)
            {
                return Ok(());
            }
        }
        let router = build_router(key, state).await?;
        let listener = tokio::net::TcpListener::bind(bind).await.map_err(|error| {
            Error::new(
                FailureCode::ServiceUnavailable,
                format!("failed to bind {} listener {bind}: {error}", key.name()),
            )
        })?;
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
                tracing::error!("[DAEMON] listener stopped: {error}");
            }
        });
        let mut slots = self.slots.lock().expect("listener slots poisoned");
        let previous = std::mem::replace(
            &mut slots[key.index()],
            Some(ListenerSlot { task, bind }),
        );
        if let Some(old) = previous {
            old.task.abort();
        }
        tracing::info!("[DAEMON] {} listening on http://{bind}", key.name());
        Ok(())
    }

    /// 停用一个面。显式关闭是意图本身，直接停，无空窗问题。
    pub fn stop(&self, key: ListenerKey) {
        let mut slots = self.slots.lock().expect("listener slots poisoned");
        if let Some(old) = slots[key.index()].take() {
            old.task.abort();
            tracing::info!("[DAEMON] {} listener stopped", key.name());
        }
    }

    pub fn shutdown_all(&self) {
        let mut slots = self.slots.lock().expect("listener slots poisoned");
        for slot in slots.iter_mut().flatten() {
            slot.task.abort();
        }
        *slots = [None, None, None, None];
    }

    pub fn snapshot(&self) -> Vec<ListenerStatus> {
        let slots = self.slots.lock().expect("listener slots poisoned");
        ALL_KEYS
            .iter()
            .map(|&key| match &slots[key.index()] {
                Some(slot) => ListenerStatus {
                    key: key.name(),
                    running: true,
                    bind: Some(slot.bind),
                },
                None => ListenerStatus {
                    key: key.name(),
                    running: false,
                    bind: None,
                },
            })
            .collect()
    }
}

/// Web 面：静态资源 + 同源 /api → App 面（App API 固定描述本机 daemon）。
pub fn web_router(state: Arc<ApiState>) -> Router {
    let api = Router::new().nest("/api", api::app_router(state));
    Router::new()
        .merge(api)
        .route("/api", any(api_not_found))
        .route("/api/*path", any(api_not_found))
        .fallback(crate::commands::web_assets::serve_react_app)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn build_router(key: ListenerKey, state: &Arc<ApiState>) -> Result<Router, Error> {
    match key {
        ListenerKey::Core => Ok(api::core_router(Arc::clone(state))),
        ListenerKey::App => Ok(api::app_router(Arc::clone(state))),
        ListenerKey::Web => Ok(web_router(Arc::clone(state))),
        ListenerKey::Aggregate => {
            let server = crate::mcp_server::McpStoreServer::from_store(
                Arc::clone(state.store()),
                ScopeRef::Store,
                None,
                None,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
            )
            .await
            .map_err(|error| {
                Error::new(
                    FailureCode::ServiceUnavailable,
                    format!("failed to build aggregate server: {error}"),
                )
            })?;
            Ok(crate::mcp_server::streamable_http_router(
                server,
                "/mcp",
            ))
        }
    }
}

pub(crate) fn resolve_bind(host: &str, port: u16) -> Result<SocketAddr, Error> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    (host, port)
        .to_socket_addrs()
        .map_err(|error| {
            Error::new(
                FailureCode::InvalidInput,
                format!("invalid server.host {host:?}: {error}"),
            )
        })?
        .next()
        .ok_or_else(|| {
            Error::new(
                FailureCode::InvalidInput,
                format!("server.host {host:?} did not resolve"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_names_and_indices_are_stable() {
        assert_eq!(ListenerKey::Core.name(), "core");
        assert_eq!(ListenerKey::App.index(), 1);
        assert_eq!(ALL_KEYS.len(), 4);
    }

    #[test]
    fn resolve_bind_accepts_ip_and_localhost() {
        assert_eq!(
            resolve_bind("127.0.0.1", 1820).unwrap(),
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 1820)
        );
        assert!(resolve_bind("localhost", 1820).is_ok());
        assert!(resolve_bind("not a host", 1820).is_err());
    }
}
