pub(crate) mod agent_projection;
pub(crate) mod codec;
pub(crate) mod collections;
pub(crate) mod entity;
pub(crate) mod event;
pub(crate) mod health;
pub(crate) mod inspect;
pub(crate) mod layer;
#[cfg(test)]
mod layer_tests;
pub(crate) mod metrics;
pub mod models;
#[cfg(feature = "redis")]
pub(crate) mod redis;
pub(crate) mod relation;
pub(crate) mod runtime;
pub mod serializer;
pub(crate) mod service_projection;
pub(crate) mod snapshot;
pub(crate) mod state;
pub(crate) mod store;
pub(crate) mod storage;
#[cfg(feature = "valkey")]
pub(crate) mod valkey;

pub use layer::{KvError, KvLayerManager, KvSnapshot, Result};
pub use metrics::KvRequestMetricsSnapshot;
pub(crate) use store::KvStore;

pub(crate) use storage::memory_kv_store;
#[cfg(feature = "redis")]
pub(crate) use storage::redis_kv_store;
#[cfg(feature = "valkey")]
pub(crate) use storage::valkey_kv_store;
