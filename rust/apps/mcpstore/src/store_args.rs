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
        help = "OpenKeyv backend name: memory, redis, valkey, postgres, sqlite, ..."
    )]
    pub backend: Option<String>,
    #[arg(
        long = "url",
        visible_alias = "redis-url",
        help = "OpenKeyv backend connection URL"
    )]
    pub backend_url: Option<String>,
    #[arg(long, help = "KV namespace")]
    pub namespace: Option<String>,
}

impl StoreSourceArgs {
    pub fn to_store_options(&self) -> StoreOptions {
        let backend = self
            .backend
            .as_ref()
            .map(|backend| CacheStorage::new(backend, self.backend_url.clone()))
            .or_else(|| {
                self.backend_url
                    .as_ref()
                    .map(|url| CacheStorage::openkeyv("redis", url))
            });

        StoreOptions {
            config_path: self.config_path.clone(),
            source_mode: match self.source {
                SourceArg::Local => SourceMode::Local,
                SourceArg::Db => SourceMode::Db,
            },
            backend,
            redis_url: self.backend_url.clone(),
            namespace: self.namespace.clone(),
        }
    }
}

pub fn build_store(source: &StoreSourceArgs) -> Result<std::sync::Arc<MCPStore>, BoxErr> {
    Ok(MCPStore::setup_with_options(source.to_store_options())?)
}
