use clap::{Args, ValueEnum};
use mcpstore::{JsonStoreConfig, SourceMode, StoreOptions};
use serde_json::Value;

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

impl StoreSourceArgs {
    /// 显式指定了任何 store 参数 → 调用方想自带 kernel（embedded）。
    pub fn is_explicit(&self) -> bool {
        self.config_path.is_some()
            || self.store.is_some()
            || self.store_config.is_some()
            || self.namespace.is_some()
            || self.source != SourceArg::Local
    }
}

/// CLI 业务命令的执行位置：daemon（默认，共享连接池与运行时）或本进程 embedded。
/// 两条路径执行同一份业务 op 分发（daemon::ops）。
pub enum StoreAccess {
    Embedded(std::sync::Arc<mcpstore::MCPStore>),
    Remote(crate::daemon::client::KernelClient),
}

/// 业务命令统一入口：默认连 daemon，不在则后台拉起；`--embedded` 或显式 store
/// 参数时本进程冷启动。kernel 的 backend/namespace 由 daemon 启动参数决定，
/// CLI 不越权覆盖。
pub async fn open_store_access(
    args: &StoreSourceArgs,
    embedded: bool,
) -> Result<StoreAccess, BoxErr> {
    if embedded || args.is_explicit() {
        return Ok(StoreAccess::Embedded(load_kernel(args).await?.store().clone()));
    }
    if !crate::daemon::ensure::is_daemon_ready().await {
        crate::daemon::ensure::spawn_detached_daemon()?;
        crate::daemon::ensure::wait_daemon_ready(std::time::Duration::from_secs(30)).await?;
    }
    Ok(StoreAccess::Remote(
        crate::daemon::client::connect_admin().await?,
    ))
}

impl StoreAccess {
    /// 执行一个请求/响应型业务 op；返回 op 的 result 载荷。
    pub async fn request(
        &mut self,
        operation: crate::daemon::protocol::KernelOperation,
        payload: Value,
    ) -> mcpstore::Result<Value> {
        match self {
            Self::Embedded(store) => {
                crate::daemon::ops::execute(store, operation, payload).await
            }
            Self::Remote(client) => client
                .request(
                    operation,
                    payload,
                    crate::daemon::protocol::DEFAULT_REQUEST_TIMEOUT,
                )
                .await
                .map(|(result, _)| result),
        }
    }

    /// embedded 侧的 store（仅流式命令在本地驱动执行时使用）。
    pub fn embedded_store(&self) -> Option<&std::sync::Arc<mcpstore::MCPStore>> {
        match self {
            Self::Embedded(store) => Some(store),
            Self::Remote(_) => None,
        }
    }

    /// remote 侧的 kernel client（仅流式命令转发事件时使用）。
    pub fn remote_client(&mut self) -> Option<&mut crate::daemon::client::KernelClient> {
        match self {
            Self::Embedded(_) => None,
            Self::Remote(client) => Some(client),
        }
    }
}
