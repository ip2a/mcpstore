use crate::identity::InstanceId;
use crate::transport::client::ConnectionPool;
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeKind {
    Startup,
    Liveness,
}

/// Result of a single probe. `latency` covers the transport request only —
/// waiting in queue behind another operation on the same connection is not
/// server latency and must not reach health windows.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProbeOutcome {
    pub succeeded: bool,
    pub latency: std::time::Duration,
}

impl ProbeOutcome {
    pub fn latency_ms(&self) -> f64 {
        self.latency.as_secs_f64() * 1000.0
    }
}

#[async_trait]
pub(crate) trait ProbeRunner: Send + Sync {
    async fn run_probe(
        &self,
        instance_id: InstanceId,
        kind: ProbeKind,
        timeout: std::time::Duration,
    ) -> ProbeOutcome;
}

#[async_trait]
impl ProbeRunner for ConnectionPool {
    async fn run_probe(
        &self,
        instance_id: InstanceId,
        _kind: ProbeKind,
        timeout: std::time::Duration,
    ) -> ProbeOutcome {
        match self.ping_elapsed(instance_id, timeout).await {
            Ok(latency) => ProbeOutcome {
                succeeded: true,
                latency,
            },
            Err(_) => ProbeOutcome {
                succeeded: false,
                latency: timeout,
            },
        }
    }
}
