use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};

use axum::Router;
use mcpstore::error::{Error, FailureCode};
use mcpstore::AppConfig;
use serde::Serialize;

use crate::commands::api::{self, ApiState};

/// HTTP faces hosted by the daemon. Kernel RPC (unix socket) is not listed here; server.rs owns it.
/// The daemon is core-only: the web panel is a separate main process (mcpstore web), gateway is a child process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerKey {
    Core,
}

const ALL_KEYS: [ListenerKey; 1] = [ListenerKey::Core];

impl ListenerKey {
    fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Core => 0,
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

/// Sole owner of listener lifecycles: start/stop, hot rebind, snapshots.
/// Hot rebind semantics: bind the new address first, then abort the old task; if bind fails the old listener keeps running.
pub struct ListenerManager {
    // ponytail: one global lock; daemon management ops arrive serially, no contention scenario
    slots: Mutex<[Option<ListenerSlot>; 1]>,
}

impl ListenerManager {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new([None]),
        }
    }

    /// Daemon startup: bring up every enabled face per config.toml.
    /// A single face failing to bind is not fatal: the kernel socket is the lifeline, so the face
    /// degrades to "not running" with a loud warning (same philosophy as hot switch keeping the old listener on failure).
    pub async fn start_all(&self, config: &AppConfig, state: &Arc<ApiState>) {
        let server = &config.server;
        for (key, port) in [(
            ListenerKey::Core,
            server.core_enabled.then_some(server.port),
        )] {
            match port {
                Some(port) => {
                    let bind = match resolve_bind(&server.host, port) {
                        Ok(bind) => bind,
                        Err(error) => {
                            eprintln!("[DAEMON] {key:?} face start skipped: {error}");
                            continue;
                        }
                    };
                    if let Err(error) = self.apply(key, bind, state).await {
                        eprintln!(
                            "[DAEMON] {} face start failed on http://{bind}: {error} (this face did not start, the rest are unaffected)",
                            key.name()
                        );
                    }
                }
                None => tracing::info!("[DAEMON] {} face disabled", key.name()),
            }
        }
    }

    /// Enable/rebind a face. Skips when the target matches the current state (idempotent); on bind
    /// failure the old listener is kept, so there is no gap.
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
        let previous =
            std::mem::replace(&mut slots[key.index()], Some(ListenerSlot { task, bind }));
        if let Some(old) = previous {
            old.task.abort();
        }
        tracing::info!("[DAEMON] {} listening on http://{bind}", key.name());
        Ok(())
    }

    /// Stop a face. An explicit close is intent in itself, so stop directly; no gap concern.
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
        *slots = [None];
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

async fn build_router(key: ListenerKey, state: &Arc<ApiState>) -> Result<Router, Error> {
    match key {
        ListenerKey::Core => Ok(api::core_router(Arc::clone(state))),
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
        assert_eq!(ALL_KEYS.len(), 1);
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
