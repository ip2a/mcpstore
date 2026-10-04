use crate::store::prelude::*;
use crate::store::ControlPlane;

impl ControlPlane {
    pub async fn add_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<()> {
        if store.definition_from_kv(service_name).await?.is_some()
            || store
                .kernel
                .control
                .config_manager
                .load_or_empty()?
                .mcp_servers
                .contains_key(service_name)
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service definition already exists: {service_name}"),
            ));
        }

        config.ensure_native_scopes();
        store
            .enqueue_service_event("add", service_name, Some(&config))
            .await
    }
}
