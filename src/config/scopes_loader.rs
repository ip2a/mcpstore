//! Compatibility shim: scopes.toml no longer carries gateway route rules.
//! Callers that still invoke `load_scopes_config` are redirected to `route.toml`.

use crate::store::prelude::*;

impl MCPStore {
    /// Deprecated name for [`MCPStore::load_route_config`].
    pub async fn load_scopes_config(&self) -> Result<()> {
        self.load_route_config().await
    }
}
