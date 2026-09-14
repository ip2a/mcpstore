use crate::control::ControlRequest;
use crate::store::{MCPStore, CONTROL_REQUEST_EVENT_TYPE};
use crate::{Error, FailureCode, Result};

impl MCPStore {
    pub async fn control_request(&self, request_id: &str) -> Result<ControlRequest> {
        let value = self
            .kernel
            .persistence
            .cache
            .get_event(CONTROL_REQUEST_EVENT_TYPE, request_id)
            .await?
            .ok_or_else(|| {
                Error::new(
                    FailureCode::InvalidInput,
                    format!("control request not found: {request_id}"),
                )
            })?;
        serde_json::from_value(value).map_err(|error| {
            Error::new(
                FailureCode::Internal,
                format!("control request deserialization failed: {error}"),
            )
        })
    }

    pub async fn control_requests(&self) -> Result<Vec<ControlRequest>> {
        let mut requests: Vec<ControlRequest> = self
            .kernel
            .persistence
            .cache
            .get_all_events_async(CONTROL_REQUEST_EVENT_TYPE)
            .await?
            .into_iter()
            .map(|(key, value)| {
                serde_json::from_value(value).map_err(|error| {
                    Error::new(
                        FailureCode::Internal,
                        format!("control request '{key}' deserialization failed: {error}"),
                    )
                })
            })
            .collect::<Result<_>>()?;
        requests.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then(left.id.cmp(&right.id))
        });
        Ok(requests)
    }

    pub fn is_control_mutation_queued(&self) -> bool {
        self.is_data_plane()
    }
}
