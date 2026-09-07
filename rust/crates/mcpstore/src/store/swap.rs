use crate::store::prelude::*;
use crate::store::store_config::StoreConfig;

/// Result of a successful `swap_store` operation.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SwapResult {
    pub source_store: String,
    pub target_store: String,
    pub copied: u64,
    pub replayed: u64,
    pub verified: bool,
    pub online: bool,
    pub pause_ms: u64,
}

impl MCPStore {
    /// Swap the active cache Store to one described by `config`.
    pub async fn swap_store(&self, config: &dyn StoreConfig) -> Result<SwapResult> {
        self.kernel.persistence.migrate_and_cutover(config).await
    }
}
