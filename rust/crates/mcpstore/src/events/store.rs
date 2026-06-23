use crate::events::{Event, EventCapabilityReport};
use crate::store::prelude::*;

impl MCPStore {
    pub async fn publish_event(
        &self,
        event_type: &str,
        payload: serde_json::Value,
        wait: bool,
    ) -> Result<()> {
        self.event_bus
            .publish(Event::new(event_type, payload), wait)
            .await;
        Ok(())
    }

    pub async fn event_history(&self, count: usize) -> Vec<Event> {
        self.event_bus.get_history(count).await
    }

    pub async fn event_capability_report(&self) -> serde_json::Value {
        let report = self.event_capability_report_entry().await;
        serde_json::json!({
            "event_bus": report.event_bus,
            "history": report.history,
            "history_capacity": report.history_capacity,
            "cache_event_layer": report.cache_event_layer,
        })
    }

    pub async fn event_capability_report_entry(&self) -> EventCapabilityReport {
        EventCapabilityReport {
            event_bus: true,
            history: true,
            history_capacity: 1000,
            cache_event_layer: true,
        }
    }
}
