#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum SourceMode {
    #[default]
    Local,
    Db,
}

use super::store_config::JsonStoreConfig;

#[derive(Clone, Debug)]
pub struct StoreOptions {
    pub config_path: Option<String>,
    pub source_mode: SourceMode,
    pub store: Option<JsonStoreConfig>,
    pub namespace: Option<String>,
    /// 节点标识：本节点状态写入与 node_status 心跳行的归属
    /// （缺省 CONTROL_NODE_ID）。
    pub node_id: Option<String>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            source_mode: SourceMode::Local,
            store: None,
            namespace: None,
            node_id: None,
        }
    }
}
