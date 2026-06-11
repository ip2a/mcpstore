#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    ZhCn,
    EnUs,
}

impl Default for Locale {
    fn default() -> Self {
        Self::ZhCn
    }
}

impl Locale {
    pub fn as_config_value(&self) -> &'static str {
        match self {
            Self::ZhCn => "zh-cn",
            Self::EnUs => "en-us",
        }
    }

    pub fn from_config_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "zh-cn" | "zh_cn" | "zh" => Some(Self::ZhCn),
            "en-us" | "en_us" | "en" => Some(Self::EnUs),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKey {
    NavServices,
    NavAddService,
    NavServiceManagement,
    NavTools,
    NavAgents,
    NavLogs,
    NavStatus,
    NavSettings,
    FocusMainNav,
    FocusControlBar,
    FocusContent,
    FilterAll,
    FilterConnected,
    FilterError,
    FilterDisconnected,
    FilterConnecting,
    SortName,
    SortStatus,
    SortTools,
    SearchPrompt,
    SearchLabel,
    SortLabel,
    ControlBarPlaceholder,
    ContentRegion,
    PlaceholderBodySuffix,
    TableName,
    TableScope,
    TableProtocol,
    TableStatus,
    TableTools,
    TableActions,
    ServiceRowActions,
    SettingsStatus,
    SettingsGeneral,
    SettingsLogging,
    SettingsControlHint,
    SettingsInstallPath,
    SettingsMcpConfigPath,
    SettingsAppConfigPath,
    SettingsConfigExists,
    SettingsRuntimeStatus,
    SettingsBackend,
    SettingsNamespace,
    SettingsSource,
    SettingsServiceCount,
    SettingsLocale,
    SettingsLocaleSource,
    SettingsConfigFile,
    SettingsServerLogLevel,
    SettingsStandaloneLogLevel,
    SettingsStandaloneLogFormat,
    SettingsDebugEnabled,
    SettingsTracingSink,
    SettingsLogFile,
    LogsControlHint,
    LogsCurrentStatus,
    LogsRecentMessages,
    LogsEmpty,
    LogsEntries,
    LogsFocus,
    LogsRuntime,
    LogsStoreEvents,
    LogsServices,
    LogsConfig,
    LogsStoreEventsEmpty,
}

pub fn text(locale: Locale, key: TextKey) -> &'static str {
    match locale {
        Locale::ZhCn => zh_cn(key),
        Locale::EnUs => en_us(key),
    }
}

fn zh_cn(key: TextKey) -> &'static str {
    match key {
        TextKey::NavServices => "服务列表",
        TextKey::NavAddService => "添加服务",
        TextKey::NavServiceManagement => "服务管理",
        TextKey::NavTools => "工具管理",
        TextKey::NavAgents => "Agent列表",
        TextKey::NavLogs => "日志",
        TextKey::NavStatus => "状态",
        TextKey::NavSettings => "设置",
        TextKey::FocusMainNav => "主导航",
        TextKey::FocusControlBar => "筛选区",
        TextKey::FocusContent => "表格区",
        TextKey::FilterAll => "全部",
        TextKey::FilterConnected => "已连接",
        TextKey::FilterError => "错误",
        TextKey::FilterDisconnected => "断开",
        TextKey::FilterConnecting => "连接中",
        TextKey::SortName => "名称",
        TextKey::SortStatus => "状态",
        TextKey::SortTools => "工具",
        TextKey::SearchPrompt => "按 / 搜索",
        TextKey::SearchLabel => "搜索",
        TextKey::SortLabel => "排序",
        TextKey::ControlBarPlaceholder => "待接入",
        TextKey::ContentRegion => "内容区",
        TextKey::PlaceholderBodySuffix => "表单区待接入",
        TextKey::TableName => "名称",
        TextKey::TableScope => "作用域",
        TextKey::TableProtocol => "协议",
        TextKey::TableStatus => "状态",
        TextKey::TableTools => "工具",
        TextKey::TableActions => "操作",
        TextKey::ServiceRowActions => "c:连 d:断 x:启 D:删",
        TextKey::SettingsStatus => "状态",
        TextKey::SettingsGeneral => "常规配置",
        TextKey::SettingsLogging => "日志配置",
        TextKey::SettingsControlHint => "h/l 切换设置项",
        TextKey::SettingsInstallPath => "安装地址",
        TextKey::SettingsMcpConfigPath => "MCP 配置",
        TextKey::SettingsAppConfigPath => "应用配置",
        TextKey::SettingsConfigExists => "配置文件",
        TextKey::SettingsRuntimeStatus => "运行状态",
        TextKey::SettingsBackend => "缓存后端",
        TextKey::SettingsNamespace => "命名空间",
        TextKey::SettingsSource => "配置来源",
        TextKey::SettingsServiceCount => "服务数量",
        TextKey::SettingsLocale => "当前语种",
        TextKey::SettingsLocaleSource => "语种来源",
        TextKey::SettingsConfigFile => "通用配置",
        TextKey::SettingsServerLogLevel => "API 日志级别",
        TextKey::SettingsStandaloneLogLevel => "独立服务日志级别",
        TextKey::SettingsStandaloneLogFormat => "独立服务日志格式",
        TextKey::SettingsDebugEnabled => "调试开关",
        TextKey::SettingsTracingSink => "TUI tracing 输出",
        TextKey::SettingsLogFile => "日志文件",
        TextKey::LogsControlHint => "自动记录最近的运行消息",
        TextKey::LogsCurrentStatus => "当前状态",
        TextKey::LogsRecentMessages => "最近消息",
        TextKey::LogsEmpty => "暂无消息",
        TextKey::LogsEntries => "条目数",
        TextKey::LogsFocus => "当前焦点",
        TextKey::LogsRuntime => "运行消息",
        TextKey::LogsStoreEvents => "Store事件",
        TextKey::LogsServices => "服务状态",
        TextKey::LogsConfig => "日志配置",
        TextKey::LogsStoreEventsEmpty => "暂无 Store 事件",
    }
}

fn en_us(key: TextKey) -> &'static str {
    match key {
        TextKey::NavServices => "Services",
        TextKey::NavAddService => "Add Service",
        TextKey::NavServiceManagement => "Service Management",
        TextKey::NavTools => "Tool Management",
        TextKey::NavAgents => "Agents",
        TextKey::NavLogs => "Logs",
        TextKey::NavStatus => "Status",
        TextKey::NavSettings => "Settings",
        TextKey::FocusMainNav => "Main nav",
        TextKey::FocusControlBar => "Control bar",
        TextKey::FocusContent => "Content",
        TextKey::FilterAll => "All",
        TextKey::FilterConnected => "Connected",
        TextKey::FilterError => "Error",
        TextKey::FilterDisconnected => "Disconnected",
        TextKey::FilterConnecting => "Connecting",
        TextKey::SortName => "Name",
        TextKey::SortStatus => "Status",
        TextKey::SortTools => "Tools",
        TextKey::SearchPrompt => "Press / to search",
        TextKey::SearchLabel => "Search",
        TextKey::SortLabel => "Sort",
        TextKey::ControlBarPlaceholder => "Pending",
        TextKey::ContentRegion => "Content",
        TextKey::PlaceholderBodySuffix => "form pending",
        TextKey::TableName => "Name",
        TextKey::TableScope => "Scope",
        TextKey::TableProtocol => "Protocol",
        TextKey::TableStatus => "Status",
        TextKey::TableTools => "Tools",
        TextKey::TableActions => "Actions",
        TextKey::ServiceRowActions => "c:on d:off x:restart D:delete",
        TextKey::SettingsStatus => "Status",
        TextKey::SettingsGeneral => "General",
        TextKey::SettingsLogging => "Logging",
        TextKey::SettingsControlHint => "h/l to switch settings",
        TextKey::SettingsInstallPath => "Install path",
        TextKey::SettingsMcpConfigPath => "MCP config",
        TextKey::SettingsAppConfigPath => "App config",
        TextKey::SettingsConfigExists => "Config file",
        TextKey::SettingsRuntimeStatus => "Runtime status",
        TextKey::SettingsBackend => "Cache backend",
        TextKey::SettingsNamespace => "Namespace",
        TextKey::SettingsSource => "Source",
        TextKey::SettingsServiceCount => "Services",
        TextKey::SettingsLocale => "Current locale",
        TextKey::SettingsLocaleSource => "Locale source",
        TextKey::SettingsConfigFile => "General config",
        TextKey::SettingsServerLogLevel => "API log level",
        TextKey::SettingsStandaloneLogLevel => "Standalone log level",
        TextKey::SettingsStandaloneLogFormat => "Standalone log format",
        TextKey::SettingsDebugEnabled => "Debug enabled",
        TextKey::SettingsTracingSink => "TUI tracing sink",
        TextKey::SettingsLogFile => "Log file",
        TextKey::LogsControlHint => "Automatically tracks recent runtime messages",
        TextKey::LogsCurrentStatus => "Current status",
        TextKey::LogsRecentMessages => "Recent messages",
        TextKey::LogsEmpty => "No messages yet",
        TextKey::LogsEntries => "Entries",
        TextKey::LogsFocus => "Focus",
        TextKey::LogsRuntime => "Runtime",
        TextKey::LogsStoreEvents => "Store events",
        TextKey::LogsServices => "Services",
        TextKey::LogsConfig => "Log config",
        TextKey::LogsStoreEventsEmpty => "No Store events",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_navigation_labels() {
        assert_eq!(text(Locale::ZhCn, TextKey::NavServices), "服务列表");
        assert_eq!(text(Locale::EnUs, TextKey::NavServices), "Services");
        assert_eq!(text(Locale::ZhCn, TextKey::NavAddService), "添加服务");
        assert_eq!(
            text(Locale::ZhCn, TextKey::NavServiceManagement),
            "服务管理"
        );
    }

    #[test]
    fn parses_config_locale_values() {
        assert_eq!(Locale::from_config_value("zh-cn"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_config_value("EN"), Some(Locale::EnUs));
        assert_eq!(Locale::from_config_value("unknown"), None);
    }
}
