use crate::cache::models::HealthStatus;
use crate::identity::InstanceId;

#[async_trait::async_trait]
pub(crate) trait SupervisorActions: Send + Sync {
    async fn apply_transition(
        &self,
        instance_id: InstanceId,
        from: HealthStatus,
        to: HealthStatus,
        reason: &'static str,
    ) -> Result<(), String>;
}
