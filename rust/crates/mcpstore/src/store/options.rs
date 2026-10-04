use super::store_config::JsonStoreConfig;

/// 面板角色：setup 的参数，决定本进程的行为。kv 永远是真源，backend
/// 只是部署参数；角色决定谁消费事件、谁执行。
///
/// - 控制面板：setup 时挂自愈监督，订 service_events / tool_call_requests
///   两条 ChangeFeed 并消费执行；有 config_path 时启动读种子、消费后落盘。
/// - 数据面板：不订、不执行；查询 kv 直读，调用按 placement 路由。
#[derive(Clone, Debug, PartialEq)]
pub enum PanelRole {
    ControlPanel,
    DataPanel { panel_id: String },
}

impl Default for PanelRole {
    fn default() -> Self {
        Self::ControlPanel
    }
}

#[derive(Clone, Debug)]
pub struct StoreOptions {
    pub config_path: Option<String>,
    pub store: Option<JsonStoreConfig>,
    pub namespace: Option<String>,
    /// 面板角色（缺省 ControlPanel：单机/云端默认都是控制面板；
    /// 数据面板必须显式声明 panel_id）。
    pub panel: PanelRole,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            config_path: None,
            store: None,
            namespace: None,
            panel: PanelRole::ControlPanel,
        }
    }
}
