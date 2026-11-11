use crate::store::prelude::*;
use crate::store::store_config::StoreSpec;

/// Result of a successful `swap_store` operation. Verification is not a flag:
/// any mismatch fails the migration outright, so a returned result is always
/// verified.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SwapResult {
    pub source_store: String,
    pub target_store: String,
    pub copied: u64,
    pub replayed: u64,
    pub online: bool,
    pub pause_ms: u64,
}

impl MCPStore {
    /// Swap the active kv store to one described by `config`.
    pub async fn swap_store(&self, config: &dyn StoreSpec) -> Result<SwapResult> {
        self.kernel.persistence.migrate_and_cutover(config).await
    }
}
