use std::sync::Arc;

#[cfg(feature = "redis")]
use crate::cache::redis_store;
use crate::cache::{memory_cache_store, CacheStore};
use crate::store::prelude::*;
use crate::store::{JsonStoreConfig, StoreConfig};

impl MCPStore {
    pub(crate) fn build_cache_store(
        store_config: &JsonStoreConfig,
        default_store_address: &str,
        _namespace: &str,
    ) -> Result<Arc<dyn CacheStore>> {
        match store_config.store_name() {
            "memory" => Ok(memory_cache_store()),
            #[cfg(feature = "redis")]
            "redis" => Ok(redis_store(
                store_config
                    .config
                    .get("url")
                    .and_then(|value| value.as_str())
                    .unwrap_or(default_store_address),
                foreign_key_policy(&store_config.config)?,
            )),
            store => Err(Error::new(
                FailureCode::Internal,
                format!(
                "OpenKeyv Store '{store}' does not provide the capabilities required by MCPStore"
            ),
            )),
        }
    }

    pub async fn current_store_name(&self) -> String {
        self.store_config.read().await.store_name().to_string()
    }
}

/// How the Redis backend treats foreign keys sharing its database.
///
/// Defaults to `skip` so shared-database deployments work out of the box;
/// set `foreign_key_policy: "strict"` in the cache store config to fail
/// loudly on non-openkeyv keys instead.
#[cfg(feature = "redis")]
fn foreign_key_policy(
    config: &serde_json::Value,
) -> Result<openkeyv::store::redis::ForeignKeyPolicy> {
    match config.get("foreign_key_policy").and_then(|value| value.as_str()) {
        None | Some("skip") => Ok(openkeyv::store::redis::ForeignKeyPolicy::Skip),
        Some("strict") => Ok(openkeyv::store::redis::ForeignKeyPolicy::Strict),
        Some(other) => Err(Error::new(
            FailureCode::ConfigInvalid,
            format!("cache store foreign_key_policy must be 'strict' or 'skip', got '{other}'"),
        )),
    }
}

#[cfg(all(test, feature = "redis"))]
mod foreign_key_policy_tests {
    use super::foreign_key_policy;
    use openkeyv::store::redis::ForeignKeyPolicy;
    use serde_json::json;

    #[test]
    fn foreign_key_policy_defaults_to_skip() {
        assert!(matches!(
            foreign_key_policy(&json!({})),
            Ok(ForeignKeyPolicy::Skip)
        ));
        assert!(matches!(
            foreign_key_policy(&json!({"foreign_key_policy": "skip"})),
            Ok(ForeignKeyPolicy::Skip)
        ));
        assert!(matches!(
            foreign_key_policy(&json!({"foreign_key_policy": "strict"})),
            Ok(ForeignKeyPolicy::Strict)
        ));
        assert!(foreign_key_policy(&json!({"foreign_key_policy": "nope"})).is_err());
    }
}
