use crate::store::prelude::*;
use crate::store::ControlPlane;

impl ControlPlane {
    pub async fn add_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<()> {
        let _kv_done = eprintln!("MARK add_service({service_name}): kv check done");
        if store.definition_from_kv(service_name).await?.is_some()
            || store.seed_file_has_service(service_name)
        {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service definition already exists: {service_name}"),
            ));
        }
        let _seed_done = eprintln!("MARK add_service({service_name}): seed check done");

        config.ensure_native_scopes();
        store
            .enqueue_service_event("add", service_name, Some(&config))
            .await
    }
}
