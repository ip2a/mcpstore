use std::sync::atomic::Ordering;

use crate::control::request;
use crate::store::prelude::*;

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

    pub(crate) async fn queue_service_add_request(
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

    pub(crate) async fn queue_control_request(
        &self,
        request_type: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        let created_at = chrono::Utc::now().timestamp_millis();
        let sequence = CONTROL_EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let event_id = format!("{request_type}:{created_at}:{sequence}");
        let dedup_key = request::dedup_key(request_type, &payload);
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
}
