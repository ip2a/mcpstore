use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use openkeyv::{AsyncChangeFeed, ChangeFeedRequest, ChangeFilter, ChangeOperation, ChangeStart};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::cache::CacheLayerManager;
use crate::store::prelude::*;
use crate::store::PanelRole;

const SERVICE_EVENTS: &str = "service_events";

impl MCPStore {
    pub(crate) async fn enqueue_service_event(
        &self,
        op: &str,
        service_name: &str,
        config: Option<&ServerConfig>,
    ) -> Result<()> {
        let mut value = serde_json::json!({
            "op": op,
            "service_name": service_name,
        });
        if let Some(config) = config {
            value["config"] = serde_json::to_value(config)
                .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        }
        self.put_service_event(value).await
    }

    pub(crate) async fn put_service_event(&self, mut value: Value) -> Result<()> {
        // Backend has no ChangeFeed: the consume loop can't start, writes would never execute — fail fast,
        // don't pretend success.
        if self
            .kernel
            .runtime
            .service_event_feed_failed
            .load(Ordering::Acquire)
        {
            return Err(Error::new(
                FailureCode::Internal,
                "backend does not provide ChangeFeed; service events cannot be consumed"
                    .to_string(),
            ));
        }
        // Stamp enqueue time: backlog replay sorts by it, preserving the user's write order.
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "enqueued_unix_ms".to_string(),
                serde_json::json!(chrono::Utc::now().timestamp_millis()),
            );
        }
        let key = uuid::Uuid::new_v4().to_string();
        self.cache().put_entity(SERVICE_EVENTS, &key, value).await?;
        // Read-your-writes: when the writer is also the executor (control panel), wait until our own event is consumed
        // before returning. Events still go through kv + the consume loop, no shortcuts. The data panel's executor is
        // elsewhere; it returns right after writing. Wait timeout is an error: either apply failed (the key is still being retried),
        // or the consume loop isn't running — in neither case should the caller see success.
        if matches!(self.kernel.runtime.panel_role, PanelRole::ControlPanel) {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                let pending = self
                    .cache()
                    .get_entity(SERVICE_EVENTS, &key)
                    .await
                    .ok()
                    .flatten()
                    .is_some();
                if !pending {
                    break;
                }
                if std::time::Instant::now() > deadline {
                    return Err(Error::new(
                        FailureCode::Internal,
                        format!(
                            "service event {key} not consumed within 2s: apply failed or consumer is not running"
                        ),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        Ok(())
    }

    /// Only control panels on a shared store subscribe. If no tokio runtime exists at setup, defer to `load_from_config`.
    pub(crate) fn spawn_service_event_feed(self: &Arc<Self>) {
        if !matches!(self.kernel.runtime.panel_role, PanelRole::ControlPanel) {
            return;
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        if self
            .kernel
            .runtime
            .service_event_feed_started
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let store = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(error) = store.run_service_event_feed().await {
                tracing::error!("[STORE] service event feed stopped: {error}");
            }
            store
                .kernel
                .runtime
                .service_event_feed_started
                .store(false, Ordering::Release);
        });
    }

    pub(crate) fn spawn_service_event_feed_from_ref(&self) {
        let Some(store) = self
            .kernel
            .runtime
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
        else {
            return;
        };
        store.spawn_service_event_feed();
    }

    async fn run_service_event_feed(&self) -> Result<()> {
        // ponytail: failed keys are rescanned every second, no extra poller. Permanently failing events retry forever; add a dead-letter queue if that bites.
        loop {
            match self.consume_service_events().await {
                Ok(()) => {}
                Err(error) if error.message().contains("does not provide ChangeFeed") => {
                    self.kernel
                        .runtime
                        .service_event_feed_failed
                        .store(true, Ordering::Release);
                    return Err(error);
                }
                Err(error) => {
                    tracing::warn!("[STORE] service event feed resubscribe: {error}");
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn consume_service_events(&self) -> Result<()> {
        let backend = self.ensure_event_backend().await?;
        let collection =
            CacheLayerManager::entity_collection_with_namespace(&self.namespace(), SERVICE_EVENTS);
        // Latest can't deliver events written before subscribing, so subscribe first, then scan existing keys.
        // For an entry seen by both, skip when the later get comes back empty.
        let mut subscription = backend
            .subscribe(ChangeFeedRequest {
                start: ChangeStart::Latest,
                filter: ChangeFilter {
                    collections: vec![collection],
                    operations: vec![ChangeOperation::Put],
                },
            })
            .await
            .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))?;
        self.drain_service_events().await;
        loop {
            tokio::select! {
                change = subscription.recv() => match change {
                    Ok(Some(change)) => self.apply_service_event(&change.key).await,
                    Ok(None) => return Ok(()),
                    Err(error) => {
                        return Err(Error::new(FailureCode::Internal, error.to_string()));
                    }
                },
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    self.drain_service_events().await;
                }
            }
        }
    }

    async fn drain_service_events(&self) {
        let events = match self.cache().get_all_entities_async(SERVICE_EVENTS).await {
            Ok(events) => events,
            Err(error) => {
                tracing::warn!("[STORE] service event drain failed: {error}");
                return;
            }
        };
        // Sort by enqueue time: backlog replay must restore write order,
        // otherwise an add+remove pair could end up in the wrong final state.
        let mut ordered = events
            .into_iter()
            .map(|(key, value)| {
                let enqueued = value
                    .get("enqueued_unix_ms")
                    .and_then(Value::as_i64)
                    .unwrap_or(i64::MIN);
                (enqueued, key)
            })
            .collect::<Vec<_>>();
        ordered.sort();
        for (_, key) in ordered {
            self.apply_service_event(&key).await;
        }
    }

    async fn apply_service_event(&self, key: &str) {
        let value = match self.cache().get_entity(SERVICE_EVENTS, key).await {
            Ok(Some(value)) => value,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!("[STORE] service event {key} unreadable: {error}");
                return;
            }
        };
        if let Err(error) = self.apply_service_event_value(&value).await {
            tracing::warn!("[STORE] service event {key} left in place: {error}");
            return;
        }
        if let Err(error) = self.cache().delete_entity(SERVICE_EVENTS, key).await {
            tracing::warn!("[STORE] service event {key} applied but not deleted: {error}");
        }
    }

    async fn apply_service_event_value(&self, value: &Value) -> Result<()> {
        let op = value.get("op").and_then(Value::as_str).unwrap_or("");
        match op {
            "add" | "update" => {
                let service_name = event_name(value)?;
                let config: ServerConfig = event_field(value, "config")?;
                self.register_configured_definition(service_name, &config)
                    .await?;
                self.connect_desired_running_instances(Some(service_name))
                    .await;
                self.sync_mcp_json(service_name, Some(&config))?;
            }
            "remove" => {
                let service_name = event_name(value)?;
                self.kernel
                    .control
                    .finish_remove_service(self, service_name)
                    .await?;
                self.sync_mcp_json(service_name, None)?;
            }
            "declare_scope" => {
                self.kernel
                    .control
                    .apply_declare_service_scope(
                        self,
                        event_name(value)?,
                        &event_field(value, "scope")?,
                        event_field(value, "descriptor")?,
                    )
                    .await?;
            }
            "remove_scope" => {
                self.kernel
                    .control
                    .apply_remove_service_scope(
                        self,
                        event_name(value)?,
                        &event_field(value, "scope")?,
                    )
                    .await?;
            }
            "reset_scope" => {
                self.kernel
                    .control
                    .apply_reset_scope(self, &event_field(value, "scope")?)
                    .await?;
            }
            "reset_config" => {
                self.kernel.control.apply_reset_config(self).await?;
            }
            other => {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("unknown service event op: {other}"),
                ));
            }
        }
        Ok(())
    }

    fn sync_mcp_json(&self, service_name: &str, config: Option<&ServerConfig>) -> Result<()> {
        if !self.kernel.runtime.sync_config_file {
            return Ok(());
        }
        let mut stored = self.kernel.control.config_manager.load_or_empty()?;
        match config {
            Some(config) => {
                stored
                    .mcp_servers
                    .insert(service_name.to_string(), config.clone());
            }
            None => {
                stored.mcp_servers.remove(service_name);
            }
        }
        self.kernel.control.config_manager.save(&stored)?;
        Ok(())
    }
}

fn event_name(value: &Value) -> Result<&str> {
    value
        .get("service_name")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            Error::new(
                FailureCode::Internal,
                "service event missing service_name".to_string(),
            )
        })
}

fn event_field<T: DeserializeOwned>(value: &Value, field: &str) -> Result<T> {
    let raw = value.get(field).cloned().ok_or_else(|| {
        Error::new(
            FailureCode::Internal,
            format!("service event missing {field}"),
        )
    })?;
    serde_json::from_value(raw)
        .map_err(|error| Error::new(FailureCode::Internal, error.to_string()))
}
