//! Transport tee: mirrors every JSON-RPC frame crossing an upstream
//! (client-role) connection into a [`FrameTelemetry`] sink. Transparent to
//! rmcp — this type itself implements `Transport<RoleClient>`, so it satisfies
//! the same blanket `IntoTransport` impl the wrapped transport did.

use std::future::Future;
use std::sync::Arc;

use rmcp::service::{RoleClient, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;

use crate::identity::InstanceId;
use crate::telemetry::{FrameRecord, FrameTelemetry};

pub(crate) struct TelemetryTransport<T> {
    inner: T,
    sink: Option<Arc<dyn FrameTelemetry>>,
    session_id: String,
    instance_id: String,
    service_name: String,
    transport_kind: &'static str,
}

impl<T> TelemetryTransport<T> {
    pub(crate) fn wrap(
        inner: T,
        sink: Option<Arc<dyn FrameTelemetry>>,
        instance_id: InstanceId,
        service_name: String,
        transport_kind: &'static str,
    ) -> Self {
        Self {
            inner,
            sink,
            session_id: uuid::Uuid::new_v4().to_string(),
            instance_id: instance_id.to_string(),
            service_name,
            transport_kind,
        }
    }

    /// Record one wire frame and bind its JSON-RPC id to the gateway trace.
    /// Telemetry failures never break the transport.
    fn emit(&self, direction: &str, message: &impl serde::Serialize) -> Option<Arc<crate::telemetry::CallTrace>> {
        let envelope = serde_json::to_value(message).ok()?;
        let call_id = envelope
            .pointer(&format!("/params/_meta/{}", crate::telemetry::CALL_ID_META_KEY.replace('/', "~1")))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let jsonrpc_id = envelope.get("id").map(|value| match value {
            serde_json::Value::String(id) => id.clone(),
            other => other.to_string(),
        });
        let trace = call_id
            .as_deref()
            .and_then(crate::telemetry::trace_for_call_id);
        if let (Some(id), Some(trace)) = (jsonrpc_id, trace.clone()) {
            crate::telemetry::bind_jsonrpc_trace(&self.instance_id, id, trace);
        }
        if let Some(sink) = &self.sink {
            let payload = envelope.to_string();
            let mut record = FrameRecord::new(self.session_id.clone(), "upstream", direction);
            record.instance_id = Some(self.instance_id.clone());
            record.peer = Some(self.service_name.clone());
            record.transport = Some(self.transport_kind.to_string());
            record.request_id = call_id;
            record.from_envelope(&envelope);
            record.payload = payload;
            record.size_bytes = record.payload.len() as i64;
            sink.record_frame(record);
        }
        trace
    }
}

impl<T> Transport<RoleClient> for TelemetryTransport<T>
where
    T: Transport<RoleClient>,
{
    type Error = T::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let trace = self.emit("up", &item);
        let mark_write = self.transport_kind == "stdio";
        let sent = self.inner.send(item);
        async move {
            let result = sent.await;
            if mark_write && result.is_ok() {
                if let Some(trace) = trace {
                    trace.mark_request_write();
                }
            }
            result
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        // The receive future borrows the inner transport, so everything the
        // emit needs must be cloned out before awaiting.
        let borrowed = self.inner.receive();
        let sink = self.sink.clone();
        let session_id = self.session_id.clone();
        let instance_id = self.instance_id.clone();
        let service_name = self.service_name.clone();
        let transport_kind = self.transport_kind;
        async move {
            let message = borrowed.await;
            if let Some(message) = message.as_ref() {
                let envelope = serde_json::to_value(message).ok();
                if let Some(envelope) = envelope {
                    let jsonrpc_id = envelope.get("id").map(|value| match value {
                        serde_json::Value::String(id) => id.clone(),
                        other => other.to_string(),
                    });
                    let trace = if envelope.get("method").is_none()
                        && (envelope.get("result").is_some() || envelope.get("error").is_some())
                    {
                        jsonrpc_id
                            .as_deref()
                            .and_then(|id| crate::telemetry::trace_for_jsonrpc(&instance_id, id))
                    } else {
                        None
                    };
                    if let Some(trace) = &trace {
                        trace.mark_response_read();
                    }
                    if let Some(sink) = sink.as_ref() {
                        let payload = envelope.to_string();
                        let mut record = FrameRecord::new(session_id, "upstream", "down");
                        record.instance_id = Some(instance_id.clone());
                        record.peer = Some(service_name);
                        record.transport = Some(transport_kind.to_string());
                        record.request_id = trace.map(|trace| trace.call_id().to_string());
                        record.from_envelope(&envelope);
                        record.payload = payload;
                        record.size_bytes = record.payload.len() as i64;
                        sink.record_frame(record);
                    }
                }
            }
            message
        }
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.inner.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct Collector(Mutex<Vec<FrameRecord>>);

    impl Collector {
        fn new() -> Self {
            Self(Mutex::new(Vec::new()))
        }

        fn frames(&self) -> Vec<FrameRecord> {
            self.0.lock().unwrap().clone()
        }
    }

    impl FrameTelemetry for Collector {
        fn record_frame(&self, record: FrameRecord) {
            self.0.lock().unwrap().push(record);
        }
    }

    /// Channel-backed fake upstream: sent frames land in `sent`, received
    /// frames are drained from `incoming`.
    struct FakeTransport {
        sent: Arc<Mutex<Vec<String>>>,
        incoming: Arc<Mutex<VecDeque<RxJsonRpcMessage<RoleClient>>>>,
    }

    impl Transport<RoleClient> for FakeTransport {
        type Error = std::io::Error;

        fn send(
            &mut self,
            item: TxJsonRpcMessage<RoleClient>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
            let sent = Arc::clone(&self.sent);
            async move {
                sent.lock()
                    .unwrap()
                    .push(serde_json::to_string(&item).unwrap());
                Ok(())
            }
        }

        fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
            let incoming = Arc::clone(&self.incoming);
            async move { incoming.lock().unwrap().pop_front() }
        }

        fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
            std::future::ready(Ok(()))
        }
    }

    fn instance() -> InstanceId {
        crate::identity::ServiceInstanceKey::new("test-service", crate::ScopeRef::Store)
            .instance_id()
    }

    #[tokio::test]
    async fn tees_both_directions_with_envelope_metadata() {
        let up: TxJsonRpcMessage<RoleClient> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "ping", "params": null
        }))
        .unwrap();
        let down: RxJsonRpcMessage<RoleClient> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": { "protocolVersion": "2026-07-28" }
        }))
        .unwrap();
        let notification: TxJsonRpcMessage<RoleClient> = serde_json::from_value(
            serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        )
        .unwrap();

        let collector = Arc::new(Collector::new());
        let fake = FakeTransport {
            sent: Arc::new(Mutex::new(Vec::new())),
            incoming: Arc::new(Mutex::new(VecDeque::from([down.clone()]))),
        };
        let mut transport = TelemetryTransport::wrap(
            fake,
            Some(collector.clone()),
            instance(),
            "test-service".to_string(),
            "stdio",
        );

        transport.send(up).await.unwrap();
        transport.send(notification).await.unwrap();
        let received = transport.receive().await;
        // The tee is transparent: the wrapped message comes through unchanged
        // (compare serialized forms — rmcp response types lack PartialEq).
        assert_eq!(
            serde_json::to_value(&received).unwrap(),
            serde_json::to_value(&Some(down)).unwrap()
        );

        let frames = collector.frames();
        assert_eq!(frames.len(), 3);

        assert_eq!(frames[0].direction, "up");
        assert_eq!(frames[0].method.as_deref(), Some("ping"));
        assert_eq!(frames[0].jsonrpc_id.as_deref(), Some("1"));
        assert_eq!(frames[0].kind, "request");
        assert_eq!(frames[0].instance_id, Some(instance().to_string()));
        assert_eq!(frames[0].peer.as_deref(), Some("test-service"));
        assert_eq!(frames[0].transport, Some("stdio".to_string()));
        assert!(frames[0].payload.contains("\"method\":\"ping\""));

        assert_eq!(frames[1].direction, "up");
        assert_eq!(
            frames[1].method.as_deref(),
            Some("notifications/initialized")
        );
        assert_eq!(frames[1].jsonrpc_id, None);
        assert_eq!(frames[1].kind, "notification");

        assert_eq!(frames[2].direction, "down");
        assert_eq!(frames[2].method, None);
        assert_eq!(frames[2].jsonrpc_id.as_deref(), Some("1"));
        assert_eq!(frames[2].kind, "response");

        // Session grouping: all three frames share one session id.
        assert_eq!(frames[0].session_id, frames[1].session_id);
        assert_eq!(frames[1].session_id, frames[2].session_id);
    }

    #[tokio::test]
    async fn correlates_request_response_frames_and_transport_completion() {
        let trace = crate::telemetry::CallTrace::register("transport-correlation-test");
        let up: TxJsonRpcMessage<RoleClient> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 77,
            "method": "tools/call",
            "params": {
                "name": "echo",
                "_meta": { (crate::telemetry::CALL_ID_META_KEY): trace.call_id() }
            }
        }))
        .unwrap();
        let down: RxJsonRpcMessage<RoleClient> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 77, "result": {"content": []}
        }))
        .unwrap();
        let server_request: RxJsonRpcMessage<RoleClient> = serde_json::from_value(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": 77,
                "method": "elicitation/create",
                "params": {"message": "input", "requestedSchema": {"type": "object"}}
            }),
        )
        .unwrap();
        let collector = Arc::new(Collector::new());
        let fake = FakeTransport {
            sent: Arc::new(Mutex::new(Vec::new())),
            incoming: Arc::new(Mutex::new(VecDeque::from([server_request, down]))),
        };
        let mut transport = TelemetryTransport::wrap(
            fake,
            Some(collector.clone()),
            instance(),
            "test-service".to_string(),
            "stdio",
        );

        transport.send(up).await.unwrap();
        transport.receive().await;
        assert!(trace.timing().response_read_ms.is_none());
        transport.receive().await;

        let timing = trace.timing();
        assert!(timing.request_write_ms.is_some());
        assert!(timing.first_byte_ms.is_none());
        assert!(timing.response_read_ms.is_some());
        let frames = collector.frames();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].request_id.as_deref(), Some(trace.call_id()));
        assert_eq!(frames[1].method.as_deref(), Some("elicitation/create"));
        assert_eq!(frames[1].request_id, None);
        assert_eq!(frames[2].request_id.as_deref(), Some(trace.call_id()));
        trace.unregister();
    }

    #[tokio::test]
    async fn passes_frames_through_when_no_sink() {
        let up: TxJsonRpcMessage<RoleClient> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "ping", "params": null
        }))
        .unwrap();
        let fake = FakeTransport {
            sent: Arc::new(Mutex::new(Vec::new())),
            incoming: Arc::new(Mutex::new(VecDeque::new())),
        };
        let sent = Arc::clone(&fake.sent);
        let mut transport = TelemetryTransport::wrap(fake, None, instance(), "s".into(), "stdio");
        transport.send(up).await.unwrap();
        assert_eq!(sent.lock().unwrap().len(), 1);
    }
}
