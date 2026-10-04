#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum SourceMode {
    #[default]
    Local,
    Db,
}

use super::store_config::JsonStoreConfig;

/// 面板角色：setup 的参数，决定本进程的行为。
///
/// - 控制面板：setup 时挂自愈监督。共享库模式下加删改只写事件，订上 ChangeFeed 后再执行。
/// - 数据面板：本地模式 load 时执行 placement。共享库模式不执行，panel_id 只是身份。
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
    pub source_mode: SourceMode,
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
            source_mode: SourceMode::Local,
            store: None,
            namespace: None,
            panel: PanelRole::ControlPanel,
        }
    }
}
