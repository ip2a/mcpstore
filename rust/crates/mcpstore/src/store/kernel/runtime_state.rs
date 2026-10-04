use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{OnceLock, RwLock as SyncRwLock, Weak};

use tokio::sync::RwLock;

use crate::identity::InstanceId;
use crate::store::options::PanelRole;
use crate::store::runtime::StoreRuntimeConfig;
use crate::store::MCPStore;

pub(crate) struct RuntimeState {
    pub(crate) namespace: SyncRwLock<String>,
    pub(crate) applied_openapi_configs:
        RwLock<HashMap<InstanceId, serde_json::Map<String, serde_json::Value>>>,
    pub(crate) local_connections: RwLock<HashSet<InstanceId>>,
    pub(crate) panel_role: PanelRole,
    pub(crate) runtime_config: StoreRuntimeConfig,
    /// 只有显式传入 config_path 才把消费结果回写 mcp.json。
    /// `ConfigManager::new()` 会解析到用户真实文件，不能当默认同步目标。
    pub(crate) sync_config_file: bool,
    pub(crate) service_event_feed_started: AtomicBool,
    /// 后端没有 ChangeFeed 时置位：写路径据此快速失败，不假装成功。
    pub(crate) service_event_feed_failed: AtomicBool,
    pub(crate) tool_call_feed_started: AtomicBool,
    pub(crate) self_weak: OnceLock<Weak<MCPStore>>,
}
