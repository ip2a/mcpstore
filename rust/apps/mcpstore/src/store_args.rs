use clap::{Args, ValueEnum};
use mcpstore::{CacheStorage, MCPStore, SourceMode, StoreOptions};

use crate::BoxErr;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum SourceArg {
    Local,
    Db,
}

impl SourceArg {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Db => "db",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CacheStorageArg {
    Memory,
    Redis,
    #[value(name = "openkeyv_memory", alias = "openkeyv-memory")]
    OpenKeyvMemory,
    #[value(name = "openkeyv_redis", alias = "openkeyv-redis")]
    OpenKeyvRedis,
}

impl CacheStorageArg {
    pub fn as_cache_storage(self) -> CacheStorage {
        match self {
            Self::Memory => CacheStorage::Memory,
            Self::Redis => CacheStorage::Redis,
            Self::OpenKeyvMemory => CacheStorage::OpenKeyvMemory,
            Self::OpenKeyvRedis => CacheStorage::OpenKeyvRedis,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct StoreSourceArgs {
    #[arg(long, help = "Config file path")]
    pub config_path: Option<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = SourceArg::Local,
        help = "Data source: local=JSON+KV, db=KV only"
    )]
    pub source: SourceArg,
    #[arg(
        long,
        value_enum,
        help = "Cache storage: memory, redis, openkeyv_memory, or openkeyv_redis"
    )]
    pub backend: Option<CacheStorageArg>,
    #[arg(
        long,
        help = "Redis URL; defaults to redis cache storage when provided"
    )]
    pub redis_url: Option<String>,
    #[arg(long, help = "KV namespace")]
    pub namespace: Option<String>,
}

impl StoreSourceArgs {
    pub fn to_store_options(&self) -> StoreOptions {
        let backend = self
            .backend
            .map(CacheStorageArg::as_cache_storage)
            .or_else(|| self.redis_url.as_ref().map(|_| CacheStorage::Redis));

        StoreOptions {
            config_path: self.config_path.clone(),
            source_mode: match self.source {
                SourceArg::Local => SourceMode::Local,
                SourceArg::Db => SourceMode::Db,
            },
            backend,
            redis_url: self.redis_url.clone(),
            namespace: self.namespace.clone(),
        }
    }
}

pub fn build_store(source: &StoreSourceArgs) -> Result<std::sync::Arc<MCPStore>, BoxErr> {
    Ok(MCPStore::setup_with_options(source.to_store_options())?)
}
