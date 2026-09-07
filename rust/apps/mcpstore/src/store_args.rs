use clap::{Args, ValueEnum};
use mcpstore::{JsonStoreConfig, SourceMode, StoreOptions};

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
        help = "Store name: memory, redis, valkey, postgres, sqlite, ..."
    )]
    pub store: Option<String>,
    #[arg(long = "store-config", help = "Store configuration JSON object")]
    pub store_config: Option<String>,
    #[arg(long, help = "KV namespace")]
    pub namespace: Option<String>,
}

impl StoreSourceArgs {
    pub fn to_store_options(&self) -> StoreOptions {
        let config = self
            .store_config
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .expect("--store-config must be a valid JSON object")
            .unwrap_or_else(|| serde_json::json!({}));
        let store = self
            .store
            .as_ref()
            .map(|store| JsonStoreConfig::new(store, config));

        StoreOptions {
            config_path: self.config_path.clone(),
            source_mode: match self.source {
                SourceArg::Local => SourceMode::Local,
                SourceArg::Db => SourceMode::Db,
            },
            node_mode: mcpstore::NodeMode::ControlPlane,
            store,
            namespace: self.namespace.clone(),
        }
    }
}

pub enum KernelBootstrap {
    Embedded(StoreOptions),
}

impl StoreSourceArgs {
    pub fn to_kernel_bootstrap(&self) -> KernelBootstrap {
        KernelBootstrap::Embedded(self.to_store_options())
    }
}

pub enum KernelHandle {
    Embedded(std::sync::Arc<mcpstore::MCPStore>),
}

pub fn boot_kernel(source: &StoreSourceArgs) -> Result<KernelHandle, BoxErr> {
    let bootstrap = source.to_kernel_bootstrap();
    let KernelBootstrap::Embedded(options) = bootstrap;
    let store = mcpstore::MCPStore::setup_with_options(options)?;
    Ok(KernelHandle::Embedded(store))
}

pub async fn load_kernel(source: &StoreSourceArgs) -> Result<KernelHandle, BoxErr> {
    let handle = boot_kernel(source)?;
    handle.load().await?;
    Ok(handle)
}

impl std::ops::Deref for KernelHandle {
    type Target = std::sync::Arc<mcpstore::MCPStore>;

    fn deref(&self) -> &Self::Target {
        self.store()
    }
}

impl KernelHandle {
    pub async fn load(&self) -> mcpstore::Result<()> {
        match self {
            Self::Embedded(store) => store.load_from_source().await,
        }
    }

    pub fn store(&self) -> &std::sync::Arc<mcpstore::MCPStore> {
        match self {
            Self::Embedded(store) => store,
        }
    }
}
