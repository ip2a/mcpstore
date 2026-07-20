use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainView {
    ServiceManagement,
    Tools,
    Agents,
    Logs,
    Status,
    Settings,
}

impl MainView {
    pub const ALL: [Self; 6] = [
        Self::ServiceManagement,
        Self::Tools,
        Self::Agents,
        Self::Logs,
        Self::Status,
        Self::Settings,
    ];

    pub fn label_key(&self) -> TextKey {
        match self {
            Self::ServiceManagement => TextKey::NavServiceManagement,
            Self::Tools => TextKey::NavTools,
            Self::Agents => TextKey::NavAgents,
            Self::Logs => TextKey::NavLogs,
            Self::Status => TextKey::NavStatus,
            Self::Settings => TextKey::NavSettings,
        }
    }

    pub fn label(&self, locale: Locale) -> &'static str {
        i18n::text(locale, self.label_key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceManagementTab {
    Services,
    AddService,
}

impl ServiceManagementTab {
    pub const ALL: [Self; 2] = [Self::Services, Self::AddService];

    pub fn label_key(&self) -> TextKey {
        match self {
            Self::Services => TextKey::NavServices,
            Self::AddService => TextKey::NavAddService,
        }
    }

    pub fn label(&self, locale: Locale) -> &'static str {
        i18n::text(locale, self.label_key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentPane {
    Menu,
    Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceListMenu {
    All,
    Stdio,
    Http,
}

impl ServiceListMenu {
    pub const ALL: [Self; 3] = [Self::All, Self::Stdio, Self::Http];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部服务",
            Self::Stdio => "stdio服务",
            Self::Http => "http服务",
        }
    }

    pub fn matches(&self, transport: &str) -> bool {
        match self {
            Self::All => true,
            Self::Stdio => transport == "stdio",
            Self::Http => transport == "streamable-http" || transport == "http",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusArea {
    MainNav,
    ViewFilter,
    ViewTable,
}

impl FocusArea {
    pub fn next(&self) -> Self {
        match self {
            Self::MainNav => Self::ViewFilter,
            Self::ViewFilter => Self::ViewTable,
            Self::ViewTable => Self::ViewTable,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Self::MainNav => Self::MainNav,
            Self::ViewFilter => Self::MainNav,
            Self::ViewTable => Self::ViewFilter,
        }
    }

    pub fn label_key(&self) -> TextKey {
        match self {
            Self::MainNav => TextKey::FocusMainNav,
            Self::ViewFilter => TextKey::FocusControlBar,
            Self::ViewTable => TextKey::FocusContent,
        }
    }

    pub fn label(&self, locale: Locale) -> &'static str {
        i18n::text(locale, self.label_key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Status,
    General,
    Logging,
}

impl SettingsSection {
    pub const ALL: [Self; 3] = [Self::Status, Self::General, Self::Logging];

    pub fn label_key(&self) -> TextKey {
        match self {
            Self::Status => TextKey::SettingsStatus,
            Self::General => TextKey::SettingsGeneral,
            Self::Logging => TextKey::SettingsLogging,
        }
    }

    pub fn label(&self, locale: Locale) -> &'static str {
        i18n::text(locale, self.label_key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPane {
    Menu,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogsPane {
    Menu,
    Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogsSection {
    Runtime,
    StoreEvents,
    Services,
    Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusSection {
    Overview,
    Cache,
    Events,
    Capabilities,
}

impl StatusSection {
    pub const ALL: [Self; 4] = [
        Self::Overview,
        Self::Cache,
        Self::Events,
        Self::Capabilities,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Overview => "运行概览",
            Self::Cache => "缓存健康",
            Self::Events => "事件能力",
            Self::Capabilities => "功能清单",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolFilterTab {
    All,
    Stdio,
    Http,
    StoreScope,
    AgentScope,
}

impl ToolFilterTab {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Stdio,
        Self::Http,
        Self::StoreScope,
        Self::AgentScope,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部工具",
            Self::Stdio => "stdio服务工具",
            Self::Http => "http服务工具",
            Self::StoreScope => "Store作用域工具",
            Self::AgentScope => "Agent作用域工具",
        }
    }

    pub fn matches(&self, service: &ServiceSummary) -> bool {
        match self {
            Self::All => true,
            Self::Stdio => service.transport == "stdio",
            Self::Http => service.transport == "streamable-http" || service.transport == "http",
            Self::StoreScope => service.scope == ScopeRef::Store,
            Self::AgentScope => matches!(service.scope, ScopeRef::Agent { .. }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolSummary {
    pub instance_id: InstanceId,
    pub name: String,
    pub service_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct AgentSummary {
    pub id: String,
    pub services: Vec<String>,
}

impl LogsSection {
    pub const ALL: [Self; 4] = [
        Self::Runtime,
        Self::StoreEvents,
        Self::Services,
        Self::Config,
    ];

    pub fn label_key(&self) -> TextKey {
        match self {
            Self::Runtime => TextKey::LogsRuntime,
            Self::StoreEvents => TextKey::LogsStoreEvents,
            Self::Services => TextKey::LogsServices,
            Self::Config => TextKey::LogsConfig,
        }
    }

    pub fn label(&self, locale: Locale) -> &'static str {
        i18n::text(locale, self.label_key())
    }
}
