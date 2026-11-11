use std::sync::{Arc, OnceLock};

use crate::kv::redis::LazyRedisStore;
use crate::kv::valkey::LazyValkeyStore;
use crate::kv::store::KvStore;
#[cfg(unix)]
use crate::kv::{KvError, Result};
use openkeyv::store::memory::MemoryStore as OpenKeyvMemoryStore;

pub(crate) fn memory_kv_store() -> Arc<KvStore> {
    let store = Arc::new(OpenKeyvMemoryStore::new());
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
    );
    Arc::new(KvStore::from_handle(handle))
}

/// Like `memory_kv_store`, but also returns the underlying MemoryStore
/// handle (sharing the same `Arc<MemoryClient>`) for use by online migration.
pub(crate) fn memory_kv_store_with_handle() -> (Arc<KvStore>, OpenKeyvMemoryStore) {
    kv_store_from_memory(OpenKeyvMemoryStore::new())
}

/// `memory-test-shared` used to get a fresh store per setup, so two in-process panels couldn't meet on a ChangeFeed.
/// ponytail: one store per process. Only for this test name; the real memory() still creates a new one each time.
pub(crate) fn shared_memory_kv_store_with_handle() -> (Arc<KvStore>, OpenKeyvMemoryStore)
{
    static SHARED: OnceLock<OpenKeyvMemoryStore> = OnceLock::new();
    kv_store_from_memory(SHARED.get_or_init(OpenKeyvMemoryStore::new).clone())
}

fn kv_store_from_memory(
    inner: OpenKeyvMemoryStore,
) -> (Arc<KvStore>, OpenKeyvMemoryStore) {
    let store = Arc::new(inner.clone());
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
    );
    (
        Arc::new(KvStore::from_handle(handle)),
        inner,
    )
}

pub(crate) fn redis_kv_store(redis_address: &str, keyspace: &str) -> Arc<KvStore> {
    let store = Arc::new(LazyRedisStore::new(redis_address, keyspace));
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
        None,
    );
    Arc::new(KvStore::from_handle(handle))
}

pub(crate) fn valkey_kv_store(valkey_address: &str, keyspace: &str) -> Arc<KvStore> {
    let store = Arc::new(LazyValkeyStore::new(valkey_address, keyspace));
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store.clone()),
        Some(store),
    );
    Arc::new(KvStore::from_handle(handle))
}

/// SHM backing store: the only store that survives process boundaries without
/// an external server. Parses the same JSON contract as the openkeyv factory
/// shm branch: `path` (required), `mode` ("create"|"open", default "open"),
/// `capacity_bytes`, `index_entries`, `change_retention`. The parent directory
/// must not be group/world-writable (openkeyv enforces this). Returns the
/// kv store plus the change-feed-capable handle for event streaming.
#[cfg(unix)]
pub(crate) fn shm_kv_store_with_handle(
    config: &serde_json::Value,
) -> Result<(Arc<KvStore>, openkeyv::StoreHandle)> {
    use openkeyv::store::shm::{ShmConfig, ShmCreateConfig, ShmOpenMode, ShmStore};

    let path = config
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| KvError::Validation("shm store config requires a 'path'".into()))?;
    let mut shm = ShmConfig::open(path);
    match config.get("mode").and_then(|v| v.as_str()) {
        None | Some("open") => {}
        Some("create") => shm.mode = ShmOpenMode::Create,
        Some(other) => {
            return Err(KvError::Validation(format!(
                "shm mode must be 'create' or 'open', got '{other}'"
            )))
        }
    }
    if let Some(capacity) = config.get("capacity_bytes").and_then(|v| v.as_u64()) {
        shm = shm.with_capacity_bytes(capacity);
    }
    if let Some(entries) = config.get("index_entries").and_then(|v| v.as_u64()) {
        shm = shm.with_index_entries(u32::try_from(entries).map_err(|_| {
            KvError::Validation(format!("shm index_entries {entries} exceeds u32"))
        })?);
    }
    if let Some(retention) = config.get("change_retention").and_then(|v| v.as_u64()) {
        shm = shm.with_change_retention(u32::try_from(retention).map_err(|_| {
            KvError::Validation(format!("shm change_retention {retention} exceeds u32"))
        })?);
    }
    let store = Arc::new(
        match shm.mode {
            ShmOpenMode::Create => ShmStore::create(
                ShmCreateConfig::new(shm.path.clone())
                    .with_capacity_bytes(shm.capacity_bytes)
                    .with_index_entries(shm.index_entries)
                    .with_change_retention(shm.change_retention),
            ),
            ShmOpenMode::Open => ShmStore::open(shm.path.clone()),
        }
        .map_err(|e| KvError::Store(format!("shm store attach failed: {e}")))?,
    );
    let feed = store
        .change_feed_enabled()
        .then(|| store.clone() as Arc<dyn openkeyv::protocol::AsyncChangeFeed>);
    let handle = openkeyv::StoreHandle::with_capabilities(
        store.clone(),
        Some(store.clone()),
        Some(store.clone()),
        Some(store.clone()),
        feed,
    );
    Ok((
        Arc::new(KvStore::from_handle(
            handle.clone(),
        )),
        handle,
    ))
}

#[cfg(all(test, unix))]
mod shm_tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    /// The shm store rejects group/world-writable parent directories, and
    /// `std::env::temp_dir()` platform semantics differ (Linux /tmp is 1777),
    /// so every test sets up its own explicit 0700 directory.
    fn shm_test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mcpstore-shm-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create shm test dir");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod 0700");
        dir
    }

    fn shm_config(path: &std::path::Path, mode: &str) -> serde_json::Value {
        json!({
            "path": path.to_str().unwrap(),
            "mode": mode,
            "capacity_bytes": 16 * 1024 * 1024,
            "index_entries": 1024,
            "change_retention": 64,
        })
    }

    #[tokio::test]
    async fn shm_create_then_reopen_roundtrip() {
        let dir = shm_test_dir();
        let path = dir.join("roundtrip.shm");
        let _ = std::fs::remove_file(&path);

        let (store, handle) =
            shm_kv_store_with_handle(&shm_config(&path, "create")).expect("create shm store");
        store
            .put("k1", json!({"v": 1}), "col")
            .await
            .expect("put");
        drop(store);
        drop(handle);

        let (store, handle) =
            shm_kv_store_with_handle(&shm_config(&path, "open")).expect("reopen shm store");
        assert_eq!(
            store.get("k1", "col").await.expect("get"),
            Some(json!({"v": 1}))
        );
        assert!(handle.capabilities.change_feed);
    }

    #[test]
    fn shm_config_rejects_missing_path_and_bad_mode() {
        assert!(shm_kv_store_with_handle(&json!({"mode": "create"})).is_err());
        assert!(
            shm_kv_store_with_handle(&json!({"path": "/tmp/mcpstore-x.shm", "mode": "nope"}))
                .is_err()
        );
    }

    /// Capacity exhaustion surfaces `StoreCapacity` loudly: the put fails,
    /// the store stays attached and usable — nothing falls back to memory.
    #[tokio::test]
    async fn shm_capacity_exhaustion_fails_loudly_without_fallback() {
        let dir = shm_test_dir();
        let path = dir.join("capacity.shm");
        let _ = std::fs::remove_file(&path);

        let mut config = shm_config(&path, "create");
        config["capacity_bytes"] = json!(16 * 1024 * 1024); // openkeyv minimum arena
        let (store, _handle) =
            shm_kv_store_with_handle(&config).expect("create shm store");

        let oversized = json!({"blob": "x".repeat(32 * 1024 * 1024)});
        let error = store
            .put("big", oversized, "col")
            .await
            .expect_err("oversized put must fail");
        assert!(
            error.to_string().contains("capacity exhausted"),
            "expected StoreCapacity, got: {error}"
        );

        // Still attached, still usable: the same handle serves normal traffic.
        store
            .put("small", json!({"v": 1}), "col")
            .await
            .expect("put after capacity failure");
        assert_eq!(
            store.get("small", "col").await.expect("get"),
            Some(json!({"v": 1}))
        );
    }

    /// Creating over a store another attach still holds returns `StoreInUse`
    /// instead of clobbering it; the error propagates (construction is loud)
    /// and the held store survives untouched.
    #[tokio::test]
    async fn shm_create_over_active_attach_fails_loudly() {
        let dir = shm_test_dir();
        let path = dir.join("inuse.shm");
        let _ = std::fs::remove_file(&path);

        let (store, holder) =
            shm_kv_store_with_handle(&shm_config(&path, "create")).expect("create shm store");
        store
            .put("marker", json!({"v": 1}), "col")
            .await
            .expect("put marker");

        let error = match shm_kv_store_with_handle(&shm_config(&path, "create")) {
            Err(error) => error,
            Ok(_) => panic!("create over an active attach must fail"),
        };
        assert!(
            error.to_string().contains("is in use"),
            "expected StoreInUse, got: {error}"
        );

        drop(store);
        drop(holder);
        // The failed create must not have damaged the held store: reopen and
        // read the original data.
        let (store, _handle) =
            shm_kv_store_with_handle(&shm_config(&path, "open")).expect("reopen shm store");
        assert_eq!(
            store.get("marker", "col").await.expect("get"),
            Some(json!({"v": 1})),
            "failed create-over-attach clobbered the held store"
        );
    }
}
