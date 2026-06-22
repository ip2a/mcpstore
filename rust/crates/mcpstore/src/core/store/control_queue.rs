use std::sync::atomic::Ordering;

use super::*;

impl MCPStore {
    pub async fn process_control_requests(&self) -> Result<usize> {
        if self.source_mode == SourceMode::Db {
            return Ok(0);
        }

        let mut events = self
            .cache
            .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| {
            let left_created = left
                .1
                .get("created_at")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default();
            let right_created = right
                .1
                .get("created_at")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default();
            left_created.cmp(&right_created).then(left.0.cmp(&right.0))
        });

        let mut processed = 0;
        for (key, mut event) in events {
            if event.get("status").and_then(serde_json::Value::as_str) != Some("pending") {
                continue;
            }

            let result = self.apply_control_request(&event).await;
            if let Some(object) = event.as_object_mut() {
                object.insert(
                    "processed_at".to_string(),
                    serde_json::json!(chrono::Utc::now().timestamp_millis()),
                );
                match result {
                    Ok(()) => {
                        object.insert("status".to_string(), serde_json::json!("completed"));
                        processed += 1;
                    }
                    Err(error) => {
                        object.insert("status".to_string(), serde_json::json!("failed"));
                        object.insert("error".to_string(), serde_json::json!(error.to_string()));
                    }
                }
            }
            self.cache
                .put_event(CONTROL_REQUEST_EVENT_TYPE, &key, event)
                .await?;
        }

        Ok(processed)
    }

    pub(super) async fn queue_service_add_request(
        &self,
        name: &str,
        original_name: &str,
        agent_id: &str,
        config: &ServerConfig,
    ) -> Result<()> {
        self.queue_control_request(
            "ServiceAddRequested",
            serde_json::json!({
                "service_name": name,
                "service_original_name": original_name,
                "agent_id": agent_id,
                "config": config,
            }),
        )
        .await
    }

    pub(super) async fn queue_control_request(
        &self,
        request_type: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        let created_at = chrono::Utc::now().timestamp_millis();
        let sequence = CONTROL_EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let event_id = format!("{request_type}:{created_at}:{sequence}");
        let dedup_key = Self::control_request_dedup_key(request_type, &payload);
        let record = serde_json::json!({
            "id": event_id.clone(),
            "type": request_type,
            "payload": payload,
            "source": "onlydb",
            "created_at": created_at,
            "dedup_key": dedup_key,
            "trace_id": event_id.clone(),
            "status": "pending",
        });
        self.cache
            .put_event(CONTROL_REQUEST_EVENT_TYPE, &event_id, record)
            .await?;
        self.event_bus
            .publish(
                Event::new(
                    request_type,
                    serde_json::json!({
                        "id": event_id.clone(),
                        "source": "onlydb",
                        "queued": true,
                    }),
                ),
                true,
            )
            .await;
        Ok(())
    }

    fn control_request_dedup_key(request_type: &str, payload: &serde_json::Value) -> String {
        let service_name = payload
            .get("service_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let agent_id = payload
            .get("agent_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        format!("{request_type}:{agent_id}:{service_name}")
    }

    async fn apply_control_request(&self, event: &serde_json::Value) -> Result<()> {
        let request_type = event
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| StoreError::Other("Control request missing type".to_string()))?;
        let payload = event
            .get("payload")
            .ok_or_else(|| StoreError::Other("Control request missing payload".to_string()))?;

        match request_type {
            "ServiceAddRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                let original_name = Self::optional_payload_string(payload, "service_original_name")
                    .unwrap_or_else(|| service_name.clone());
                let agent_id = Self::optional_payload_string(payload, "agent_id")
                    .unwrap_or_else(|| GLOBAL_AGENT_STORE.to_string());
                let config = Self::required_payload_config(payload)?;
                self.add_service_with_identity(&service_name, &original_name, &agent_id, config)
                    .await?;
            }
            "ServiceUpdateRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.update_service(&service_name, Self::required_payload_config(payload)?)
                    .await?;
            }
            "ServicePatchRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                let updates = payload.get("updates").cloned().ok_or_else(|| {
                    StoreError::Other("Control request missing updates".to_string())
                })?;
                self.patch_service(&service_name, updates).await?;
            }
            "ServiceRemoveRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.remove_service(&service_name).await?;
            }
            "ServiceAssignRequested" => {
                let agent_id = Self::required_payload_string(payload, "agent_id")?;
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.assign_service_to_agent(&agent_id, &service_name)
                    .await?;
            }
            "ServiceUnassignRequested" => {
                let agent_id = Self::required_payload_string(payload, "agent_id")?;
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.unassign_service_from_agent(&agent_id, &service_name)
                    .await?;
            }
            "ServiceConnectRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.connect_service_internal(&service_name, false).await?;
            }
            "ServiceDisconnectRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.disconnect_service(&service_name).await?;
            }
            "ServiceRestartRequested" => {
                let service_name = Self::required_payload_string(payload, "service_name")?;
                self.restart_service(&service_name).await?;
            }
            "StoreResetRequested" => {
                self.reset_config().await?;
            }
            other => {
                return Err(StoreError::Other(format!(
                    "Unsupported control request type: {other}"
                )));
            }
        }
        Ok(())
    }

    fn required_payload_string(payload: &serde_json::Value, field: &str) -> Result<String> {
        payload
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| StoreError::Other(format!("Control request missing {field}")))
    }

    fn optional_payload_string(payload: &serde_json::Value, field: &str) -> Option<String> {
        payload
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    }

    fn required_payload_config(payload: &serde_json::Value) -> Result<ServerConfig> {
        let config = payload
            .get("config")
            .cloned()
            .ok_or_else(|| StoreError::Other("Control request missing config".to_string()))?;
        serde_json::from_value(config).map_err(|error| {
            StoreError::Other(format!(
                "Control request config deserialization failed: {error}"
            ))
        })
    }
}
