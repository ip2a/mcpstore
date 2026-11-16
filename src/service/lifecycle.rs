use crate::config::ScopeDescriptor;
use crate::store::prelude::*;
use crate::store::ControlPlane;

impl ControlPlane {
    /// Create-only: fails when a definition already exists. Used by imports and
    /// other paths that must not silently attach scopes.
    pub async fn add_service(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<()> {
        if store.definition_from_kv(service_name).await?.is_some()
            || store.seed_file_has_service(service_name)
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

    /// Add a service into the scopes declared in `config._mcpstore.scopes`.
    ///
    /// - No definition yet → create it (same as [`Self::add_service`]).
    /// - Definition exists with the same base config → declare any missing scopes.
    /// - Definition exists with a different base config → error.
    /// - All requested scopes already declared → error.
    ///
    /// This is the product "add service" path (HTTP `POST /services/:name` and
    /// the SDK facade). Keep [`Self::add_service`] for create-only callers.
    pub async fn add_service_into_scopes(
        &self,
        store: &MCPStore,
        service_name: &str,
        mut config: ServerConfig,
    ) -> Result<()> {
        config.ensure_native_scopes();
        let requested = config.scopes();

        if let Some(existing) = store.definition_server_config(service_name).await? {
            if existing.base_config() != config.base_config() {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!(
                        "Service definition already exists with a different base config: {service_name}"
                    ),
                ));
            }

            let mut attached = 0usize;
            for scope in requested.scopes() {
                if existing.scopes().descriptor(&scope).is_some() {
                    continue;
                }
                let descriptor = requested
                    .descriptor(&scope)
                    .cloned()
                    .unwrap_or_else(ScopeDescriptor::default);
                self.declare_service_scope(store, service_name, &scope, descriptor)
                    .await?;
                attached += 1;
            }

            if attached == 0 {
                return Err(Error::new(
                    FailureCode::Internal,
                    format!("Service '{service_name}' is already declared for the requested scope(s)"),
                ));
            }
            return Ok(());
        }

        if store.seed_file_has_service(service_name) {
            return Err(Error::new(
                FailureCode::Internal,
                format!("Service definition already exists: {service_name}"),
            ));
        }

        store
            .enqueue_service_event("add", service_name, Some(&config))
            .await
    }
}
