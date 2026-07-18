use std::sync::atomic::Ordering;

use crate::control::request::{self, ControlRequest, ControlRequestStatus};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn process_control_requests(&self) -> Result<usize> {
        if self.source_mode == SourceMode::Db {
            return Ok(0);
        }

        let mut requests = self
            .cache
            .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
            .await?
            .into_iter()
            .map(|(key, value)| {
                let request = serde_json::from_value::<ControlRequest>(value).map_err(|error| {
                    StoreError::Other(format!(
                        "Control request '{key}' deserialization failed: {error}"
                    ))
                })?;
                Ok((key, request))
            })
            .collect::<Result<Vec<_>>>()?;
        requests.sort_by(|left, right| {
            left.1
                .created_at
                .cmp(&right.1.created_at)
                .then(left.0.cmp(&right.0))
        });

        let mut processed = 0;
        for (key, mut request) in requests {
            if request.status != ControlRequestStatus::Queued {
                continue;
            }

            request.status = ControlRequestStatus::Executing {
                started_at: chrono::Utc::now().timestamp_millis(),
            };
            self.cache
                .put_event(
                    CONTROL_REQUEST_EVENT_TYPE,
                    &key,
                    serde_json::to_value(&request)
                        .map_err(|error| StoreError::Other(error.to_string()))?,
                )
                .await?;

            match self.apply_control_request(&request).await {
                Ok(()) => {
                    request.status = ControlRequestStatus::Applied {
                        applied_at: chrono::Utc::now().timestamp_millis(),
                    };
                    processed += 1;
                }
                Err(error) => {
                    request.status = ControlRequestStatus::Rejected {
                        rejected_at: chrono::Utc::now().timestamp_millis(),
                        reason: error.to_string(),
                    };
                }
            }
            self.cache
                .put_event(
                    CONTROL_REQUEST_EVENT_TYPE,
                    &key,
                    serde_json::to_value(request)
                        .map_err(|error| StoreError::Other(error.to_string()))?,
                )
                .await?;
        }

        Ok(processed)
    }

    pub(crate) async fn queue_control_request(
        &self,
        request_type: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        let created_at = chrono::Utc::now().timestamp_millis();
        let sequence = CONTROL_EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let event_id = format!("{request_type}:{created_at}:{sequence}");
        let request = ControlRequest {
            id: event_id.clone(),
            request_type: request_type.to_string(),
            dedup_key: request::dedup_key(request_type, &payload),
            payload,
            source: "onlydb".to_string(),
            created_at,
            trace_id: event_id.clone(),
            status: ControlRequestStatus::Queued,
        };
        self.cache
            .put_event(
                CONTROL_REQUEST_EVENT_TYPE,
                &event_id,
                serde_json::to_value(request)
                    .map_err(|error| StoreError::Other(error.to_string()))?,
            )
            .await?;
        self.event_bus
            .publish(
                Event::new(
                    request_type,
                    serde_json::json!({
                        "id": event_id,
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
