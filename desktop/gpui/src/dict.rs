//! Minimal en/zh dictionary. `Lang` lives in a gpui Global; views call
//! `t(cx, Key::…)`. Dynamic backend messages stay server-localized.

use gpui_kit::{App, Global, UpdateGlobal as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Auto,
    En,
    Zh,
}

impl Lang {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("en") => Self::En,
            Some("zh") => Self::Zh,
            _ => Self::Auto,
        }
    }

    fn zh(self) -> bool {
        match self {
            Self::Zh => true,
            Self::En => false,
            // auto: no locale detection wired yet; default to Chinese UI like the
            // primary deployment. ponytail: pick up system locale if needed.
            Self::Auto => true,
        }
    }
}

/// Resolved language for rendering. `Auto` resolves here once, at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiLang(pub Lang);

impl Global for UiLang {}

pub fn init(lang: Lang, cx: &mut App) {
    cx.set_global(UiLang(lang));
}

pub fn current(cx: &App) -> Lang {
    cx.global::<UiLang>().0
}

pub fn set(lang: Lang, cx: &mut App) {
    UiLang::update_global(cx, |ui, _| ui.0 = lang);
}

#[derive(Clone, Copy)]
pub enum Key {
    Services,
    Scopes,
    Tools,
    Cache,
    Settings,
    Refresh,
    Add,
    AddService,
    AddHost,
    Edit,
    Remove,
    RemoveScopeTitle,
    RemoveScopeBody,
    Connect,
    Disconnect,
    Restart,
    Details,
    Run,
    Read,
    Save,
    Clear,
    Use,
    Declare,
    DeclareService,
    ResetConfig,
    ToggleTheme,
    StartAuth,
    RefreshToken,
    Logout,
    Name,
    Transport,
    CommandOrUrl,
    Description,
    ServiceName,
    Target,
    Version,
    Language,
    Hosts,
    HostsHint,
    Active,
    Health,
    Inspect,
    SwitchBackend,
    Overview,
    Resources,
    Prompts,
    Auth,
    AuthStatus,
    RequiredScope,
    Result,
    InstanceId,
    Scope,
    Desired,
    Phase,
    ToolsSync,
    Readiness,
    Loading,
    RunningDots,
    Saving,
    Declaring,
    Reading,
    NoServices,
    SearchServices,
    SearchTools,
    ToolTakesNoArgs,
    Saved,
    PolicySaved,
    Declared,
    ServiceAdded,
    ToolsCount,
    Instances,
    ToolPolicy,
    PolicyHint,
    Empty,
    Service,
    ControlPanel,
    McpHub,
    Start,
    Stop,
    ViewConfig,
    ImportClient,
    Overrides,
    KeyLabel,
    Enable,
    Disable,
    AddOverride,
}

pub fn t(cx: &App, key: Key) -> &'static str {
    let lang = current(cx);
    let zh = lang.zh();
    match (key, zh) {
        (Key::Services, false) => "Services",
        (Key::Services, true) => "服务",
        (Key::Scopes, false) => "Scopes",
        (Key::Scopes, true) => "作用域",
        (Key::Tools, false) => "Tools",
        (Key::Tools, true) => "工具",
        (Key::Cache, false) => "Cache",
        (Key::Cache, true) => "缓存",
        (Key::Settings, false) => "Settings",
        (Key::Settings, true) => "设置",
        (Key::Refresh, false) => "Refresh",
        (Key::Refresh, true) => "刷新",
        (Key::Add, false) => "Add",
        (Key::Add, true) => "添加",
        (Key::AddService, false) => "Add service",
        (Key::AddService, true) => "添加服务",
        (Key::AddHost, false) => "Add host",
        (Key::AddHost, true) => "添加主机",
        (Key::Edit, false) => "Edit",
        (Key::Edit, true) => "编辑",
        (Key::Remove, false) => "Remove scope",
        (Key::Remove, true) => "移除作用域",
        (Key::RemoveScopeTitle, false) => "Remove from scope?",
        (Key::RemoveScopeTitle, true) => "从作用域移除？",
        (Key::RemoveScopeBody, false) => {
            "This removes the scope declaration; the service config stays on disk."
        }
        (Key::RemoveScopeBody, true) => "仅移除作用域声明，服务配置仍保留在磁盘上。",
        (Key::Connect, false) => "Connect",
        (Key::Connect, true) => "连接",
        (Key::Disconnect, false) => "Disconnect",
        (Key::Disconnect, true) => "断开",
        (Key::Restart, false) => "Restart",
        (Key::Restart, true) => "重启",
        (Key::Details, false) => "Details",
        (Key::Details, true) => "详情",
        (Key::Run, false) => "Run",
        (Key::Run, true) => "运行",
        (Key::Read, false) => "Read",
        (Key::Read, true) => "读取",
        (Key::Save, false) => "Save",
        (Key::Save, true) => "保存",
        (Key::Clear, false) => "Clear",
        (Key::Clear, true) => "清除",
        (Key::Use, false) => "Use",
        (Key::Use, true) => "使用",
        (Key::Declare, false) => "Declare",
        (Key::Declare, true) => "声明",
        (Key::DeclareService, false) => "Declare service",
        (Key::DeclareService, true) => "声明服务",
        (Key::ResetConfig, false) => "Reset config",
        (Key::ResetConfig, true) => "重置配置",
        (Key::ToggleTheme, false) => "Toggle theme",
        (Key::ToggleTheme, true) => "切换主题",
        (Key::StartAuth, false) => "Start authorization",
        (Key::StartAuth, true) => "开始授权",
        (Key::RefreshToken, false) => "Refresh token",
        (Key::RefreshToken, true) => "刷新令牌",
        (Key::Logout, false) => "Logout",
        (Key::Logout, true) => "登出",
        (Key::Name, false) => "Name",
        (Key::Name, true) => "名称",
        (Key::Transport, false) => "Transport",
        (Key::Transport, true) => "传输方式",
        (Key::CommandOrUrl, false) => "Command or URL",
        (Key::CommandOrUrl, true) => "命令或 URL",
        (Key::Description, false) => "Description",
        (Key::Description, true) => "描述",
        (Key::ServiceName, false) => "Service name",
        (Key::ServiceName, true) => "服务名",
        (Key::Target, false) => "Target",
        (Key::Target, true) => "目标",
        (Key::Version, false) => "Version",
        (Key::Version, true) => "版本",
        (Key::Language, false) => "Language",
        (Key::Language, true) => "语言",
        (Key::Hosts, false) => "Hosts (core backend)",
        (Key::Hosts, true) => "主机（core 后端）",
        (Key::HostsHint, false) => "Switch which backend the panel talks to.",
        (Key::HostsHint, true) => "切换控制面板连接的后端。",
        (Key::Active, false) => "active",
        (Key::Active, true) => "当前",
        (Key::Health, false) => "Health",
        (Key::Health, true) => "健康",
        (Key::Inspect, false) => "Inspect",
        (Key::Inspect, true) => "检查",
        (Key::SwitchBackend, false) => "Switch backend",
        (Key::SwitchBackend, true) => "切换后端",
        (Key::Overview, false) => "Overview",
        (Key::Overview, true) => "概览",
        (Key::Resources, false) => "Resources",
        (Key::Resources, true) => "资源",
        (Key::Prompts, false) => "Prompts",
        (Key::Prompts, true) => "提示词",
        (Key::Auth, false) => "Auth",
        (Key::Auth, true) => "认证",
        (Key::AuthStatus, false) => "Auth status",
        (Key::AuthStatus, true) => "认证状态",
        (Key::RequiredScope, false) => "Required scope",
        (Key::RequiredScope, true) => "所需权限",
        (Key::Result, false) => "Result",
        (Key::Result, true) => "结果",
        (Key::InstanceId, false) => "Instance ID",
        (Key::InstanceId, true) => "实例 ID",
        (Key::Scope, false) => "Scope",
        (Key::Scope, true) => "作用域",
        (Key::Desired, false) => "Desired",
        (Key::Desired, true) => "期望状态",
        (Key::Phase, false) => "Phase",
        (Key::Phase, true) => "运行阶段",
        (Key::ToolsSync, false) => "Tools sync",
        (Key::ToolsSync, true) => "工具同步",
        (Key::Readiness, false) => "Readiness",
        (Key::Readiness, true) => "就绪",
        (Key::Loading, false) => "Loading…",
        (Key::Loading, true) => "加载中…",
        (Key::RunningDots, false) => "Running…",
        (Key::RunningDots, true) => "运行中…",
        (Key::Saving, false) => "Saving…",
        (Key::Saving, true) => "保存中…",
        (Key::Declaring, false) => "Declaring…",
        (Key::Declaring, true) => "声明中…",
        (Key::Reading, false) => "Reading…",
        (Key::Reading, true) => "读取中…",
        (Key::NoServices, false) => "No services",
        (Key::NoServices, true) => "暂无服务",
        (Key::SearchServices, false) => "Search services…",
        (Key::SearchServices, true) => "搜索服务…",
        (Key::SearchTools, false) => "Search tools…",
        (Key::SearchTools, true) => "搜索工具…",
        (Key::ToolTakesNoArgs, false) => "This tool takes no arguments.",
        (Key::ToolTakesNoArgs, true) => "该工具无需参数。",
        (Key::Saved, false) => "Saved. The list refreshes within 5 seconds.",
        (Key::Saved, true) => "已保存，列表将在 5 秒内刷新。",
        (Key::PolicySaved, false) => "Policy saved.",
        (Key::PolicySaved, true) => "策略已保存。",
        (Key::Declared, false) => "Declared. Refresh the scope view to see it.",
        (Key::Declared, true) => "已声明，刷新作用域视图即可看到。",
        (Key::ServiceAdded, false) => "Service added. The list refreshes within 5 seconds.",
        (Key::ServiceAdded, true) => "服务已添加，列表将在 5 秒内刷新。",
        (Key::ToolsCount, false) => "tools",
        (Key::ToolsCount, true) => "工具",
        (Key::Instances, false) => "instances",
        (Key::Instances, true) => "实例",
        (Key::ToolPolicy, false) => "Tool policy",
        (Key::ToolPolicy, true) => "工具策略",
        (Key::PolicyHint, false) => {
            "One tool name per line. Clear to remove the override."
        }
        (Key::PolicyHint, true) => "每行一个工具名；清除即可取消覆盖。",
        (Key::Empty, false) => "—",
        (Key::Empty, true) => "—",
        (Key::Service, false) => "Service",
        (Key::Service, true) => "服务",
        (Key::ControlPanel, false) => "control panel",
        (Key::ControlPanel, true) => "控制面板",
        (Key::McpHub, false) => "MCP Hub",
        (Key::McpHub, true) => "MCP Hub",
        (Key::Start, false) => "Start",
        (Key::Start, true) => "启动",
        (Key::Stop, false) => "Stop",
        (Key::Stop, true) => "停止",
        (Key::ViewConfig, false) => "View config",
        (Key::ViewConfig, true) => "查看配置",
        (Key::ImportClient, false) => "Import client services",
        (Key::ImportClient, true) => "导入客户端服务",
        (Key::Overrides, false) => "Overrides",
        (Key::Overrides, true) => "覆盖",
        (Key::KeyLabel, false) => "Key",
        (Key::KeyLabel, true) => "键",
        (Key::Enable, false) => "Enable",
        (Key::Enable, true) => "启用",
        (Key::Disable, false) => "Disable",
        (Key::Disable, true) => "停用",
        (Key::AddOverride, false) => "Add override",
        (Key::AddOverride, true) => "添加覆盖",
    }
}

/// `t(cx, key)` shorthand.
#[macro_export]
macro_rules! t {
    ($cx:expr, $key:expr) => {
        $crate::dict::t($cx, $key)
    };
}
