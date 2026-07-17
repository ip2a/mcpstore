use crate::identity::InstanceId;
use crate::transport::client::ConnectionPool;
use crate::transport::Result;
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeKind {
    Liveness,
}

#[async_trait]
pub(crate) trait ProbeRunner: Send + Sync {
    async fn run_probe(
        &self,
        instance_id: InstanceId,
        kind: ProbeKind,
        timeout: std::time::Duration,
    ) -> Result<()>;
}

#[async_trait]
impl ProbeRunner for ConnectionPool {
    async fn run_probe(
        &self,
        instance_id: InstanceId,
        _kind: ProbeKind,
        timeout: std::time::Duration,
    ) -> Result<()> {
        self.ping(instance_id, timeout).await
    }
}
