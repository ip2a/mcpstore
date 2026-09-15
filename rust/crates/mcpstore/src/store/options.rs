#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum SourceMode {
    #[default]
    Local,
    Db,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum NodeMode {
    #[default]
    ControlPlane,
    DataPlane,
}
use super::store_config::JsonStoreConfig;

#[derive(Clone, Debug)]
pub struct StoreOptions {
    pub config_path: Option<String>,
    pub source_mode: SourceMode,
    pub node_mode: NodeMode,
    pub store: Option<JsonStoreConfig>,
    pub namespace: Option<String>,
    /// 状态分栏的节点标识：每个节点只写自己的栏（`instance@node`），
    /// 缺省 ControlPlane -> "control"（权威栏），DataPlane -> "data"。
    pub node_id: Option<String>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            source_mode: SourceMode::Local,
            node_mode: NodeMode::ControlPlane,
            store: None,
            namespace: None,
            node_id: None,
        }
    }
}
