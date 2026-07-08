use std::collections::{HashMap, VecDeque};
use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mcpstore::{config::ServerConfig, registry::ConnectionStatus, transport::ContentItem};
use ratatui::{backend::CrosstermBackend, widgets::TableState, Terminal};

use super::i18n::{self, Locale, TextKey};
use super::pages;
use super::widgets::filter_bar::{FilterBarState, FilterStatus};
use super::widgets::header::HeaderStats;
use super::widgets::service_table::{filter_and_sort, ServiceSummary};
use crate::{bootstrap, store_args::StoreSourceArgs, BoxErr};

#[derive(Clone)]
pub struct SelectedDetail {
    pub title: String,
    pub transport: String,
    pub endpoint: String,
    pub scope: String,
    pub added_time: String,
    pub connection_status: String,
    pub health_status: String,
    pub attempts: String,
    pub latency: String,
    pub retry_time: String,
    pub error_message: String,
    pub tools: Vec<String>,
}

#[derive(Clone)]
pub enum PendingAction {
    Remove(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditTarget {
    Locale,
    AddServiceField(AddServiceField),
    ToolTestArgs,
    AgentId,
    AgentAssignService,
}

#[derive(Clone, Debug)]
pub struct EditModalState {
    pub target: EditTarget,
    pub title: String,
    pub value: String,
    pub hint: String,
}

#[derive(Clone, Debug)]
pub struct SelectModalState {
    pub target: EditTarget,
    pub title: String,
    pub options: Vec<String>,
    pub selected: usize,
}

#[derive(Clone, Debug)]
pub struct LoadingModalState {
    pub title: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingTask {
    AddService,
    RefreshTools,
    ToolTest,
    RefreshAgents,
    AssignAgentService,
    UnassignAgentService,
}

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
            Self::Http => {
                transport == "streamable-http" || transport == "http" || transport == "sse"
            }
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
            Self::Http => {
                service.transport == "streamable-http"
                    || service.transport == "http"
                    || service.transport == "sse"
            }
            Self::StoreScope => service.agent_id == "global_agent_store",
            Self::AgentScope => service.agent_id != "global_agent_store",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolSummary {
    pub name: String,
    pub original_name: String,
    pub service_name: String,
    pub global_service_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolSummary {
    fn from_payload(value: serde_json::Value) -> Option<Self> {
        Some(Self {
            name: value.get("name")?.as_str()?.to_string(),
            original_name: value
                .get("original_name")
                .and_then(|v| v.as_str())
                .or_else(|| value.get("name").and_then(|v| v.as_str()))?
                .to_string(),
            service_name: value
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            global_service_name: value
                .get("global_service_name")
                .or_else(|| value.get("service_global_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string(),
            description: value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            input_schema: value
                .get("input_schema")
                .or_else(|| value.get("schema"))
                .or_else(|| value.get("inputSchema"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        })
    }
}

#[derive(Clone, Debug)]
pub struct AgentSummary {
    pub id: String,
    pub services: Vec<String>,
}

impl AgentSummary {
    fn from_value(value: serde_json::Value) -> Option<Self> {
        let id = value
            .get("agent_id")
            .or_else(|| value.get("id"))
            .and_then(|v| v.as_str())?
            .to_string();
        let services = value
            .get("services")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Some(Self { id, services })
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServiceMode {
    Stdio,
    Http,
    Json,
    Toml,
}

impl AddServiceMode {
    pub const ALL: [Self; 4] = [Self::Stdio, Self::Http, Self::Json, Self::Toml];
    pub const MENU: [Self; 4] = [Self::Http, Self::Stdio, Self::Json, Self::Toml];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
            Self::Json => "json",
            Self::Toml => "toml",
        }
    }

    pub fn menu_label(&self) -> &'static str {
        match self {
            Self::Http => "添加http服务",
            Self::Stdio => "添加stdio服务",
            Self::Json => "从json添加",
            Self::Toml => "从toml添加",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServiceField {
    Name,
    Command,
    Args,
    Url,
    Description,
    WorkingDir,
    Env,
    Headers,
    Scope,
    Agent,
    ConnectAfterAdd,
    Json,
    Toml,
    Submit,
}

impl AddServiceField {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Command => "Command",
            Self::Args => "Args",
            Self::Url => "URL",
            Self::Description => "Description",
            Self::WorkingDir => "Working directory",
            Self::Env => "Env vars",
            Self::Headers => "Headers",
            Self::Scope => "Scope",
            Self::Agent => "Agent ID",
            Self::ConnectAfterAdd => "Connect after add",
            Self::Json => "ServerConfig JSON",
            Self::Toml => "ServerConfig TOML",
            Self::Submit => "Add service",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServicePane {
    Menu,
    Form,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServiceSection {
    Basic,
    Connection,
    Scope,
    Advanced,
    Submit,
}

impl AddServiceSection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Basic => "基础信息",
            Self::Connection => "连接配置",
            Self::Scope => "作用域",
            Self::Advanced => "高级配置",
            Self::Submit => "提交",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AddServiceFormState {
    pub mode: AddServiceMode,
    pub pane: AddServicePane,
    pub selected_section: usize,
    pub selected_field: usize,
    pub name: String,
    pub command: String,
    pub args: String,
    pub url: String,
    pub description: String,
    pub working_dir: String,
    pub env: String,
    pub headers: String,
    pub scope: String,
    pub agent: String,
    pub connect_after_add: String,
    pub json: String,
    pub toml: String,
}

impl Default for AddServiceFormState {
    fn default() -> Self {
        Self {
            mode: AddServiceMode::Http,
            pane: AddServicePane::Menu,
            selected_section: 0,
            selected_field: 0,
            name: String::new(),
            command: "npx -y @modelcontextprotocol/server-filesystem .".to_string(),
            args: String::new(),
            url: "http://127.0.0.1:8000/mcp".to_string(),
            description: String::new(),
            working_dir: String::new(),
            env: String::new(),
            headers: String::new(),
            scope: "store".to_string(),
            agent: String::new(),
            connect_after_add: "yes".to_string(),
            json: "{ \"command\": \"npx\", \"args\": [\"-y\", \"@modelcontextprotocol/server-filesystem\", \".\"], \"transport\": \"stdio\" }".to_string(),
            toml: "command = \"npx\"\nargs = [\"-y\", \"@modelcontextprotocol/server-filesystem\", \".\"]\ntransport = \"stdio\"".to_string(),
        }
    }
}

impl AddServiceFormState {
    pub fn fields(&self) -> &'static [AddServiceField] {
        match self.mode {
            AddServiceMode::Stdio => &[
                AddServiceField::Name,
                AddServiceField::Description,
                AddServiceField::Command,
                AddServiceField::Args,
                AddServiceField::WorkingDir,
                AddServiceField::Env,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
            AddServiceMode::Http => &[
                AddServiceField::Name,
                AddServiceField::Description,
                AddServiceField::Url,
                AddServiceField::Headers,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
            AddServiceMode::Json => &[
                AddServiceField::Name,
                AddServiceField::Json,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
            AddServiceMode::Toml => &[
                AddServiceField::Name,
                AddServiceField::Toml,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
        }
    }

    pub fn sections(&self) -> &'static [AddServiceSection] {
        match self.mode {
            AddServiceMode::Stdio => &[
                AddServiceSection::Basic,
                AddServiceSection::Connection,
                AddServiceSection::Scope,
                AddServiceSection::Advanced,
                AddServiceSection::Submit,
            ],
            AddServiceMode::Http => &[
                AddServiceSection::Basic,
                AddServiceSection::Connection,
                AddServiceSection::Scope,
                AddServiceSection::Submit,
            ],
            AddServiceMode::Json | AddServiceMode::Toml => &[
                AddServiceSection::Basic,
                AddServiceSection::Connection,
                AddServiceSection::Scope,
                AddServiceSection::Submit,
            ],
        }
    }

    pub fn selected_section(&self) -> AddServiceSection {
        self.sections()
            .get(self.selected_section)
            .copied()
            .unwrap_or(AddServiceSection::Basic)
    }

    pub fn fields_for_section(&self, section: AddServiceSection) -> &'static [AddServiceField] {
        match (self.mode, section) {
            (AddServiceMode::Stdio | AddServiceMode::Http, AddServiceSection::Basic) => {
                &[AddServiceField::Name, AddServiceField::Description]
            }
            (AddServiceMode::Json | AddServiceMode::Toml, AddServiceSection::Basic) => {
                &[AddServiceField::Name]
            }
            (AddServiceMode::Stdio, AddServiceSection::Connection) => {
                &[AddServiceField::Command, AddServiceField::Args]
            }
            (AddServiceMode::Http, AddServiceSection::Connection) => {
                &[AddServiceField::Url, AddServiceField::Headers]
            }
            (AddServiceMode::Json, AddServiceSection::Connection) => &[AddServiceField::Json],
            (AddServiceMode::Toml, AddServiceSection::Connection) => &[AddServiceField::Toml],
            (_, AddServiceSection::Scope) => &[
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
            ],
            (AddServiceMode::Stdio, AddServiceSection::Advanced) => {
                &[AddServiceField::WorkingDir, AddServiceField::Env]
            }
            (AddServiceMode::Http, AddServiceSection::Advanced) => &[],
            (_, AddServiceSection::Advanced) => &[],
            (_, AddServiceSection::Submit) => &[AddServiceField::Submit],
        }
    }

    pub fn selected_fields(&self) -> &'static [AddServiceField] {
        self.fields()
    }

    pub fn selected_field(&self) -> AddServiceField {
        self.selected_fields()
            .get(self.selected_field)
            .copied()
            .unwrap_or(AddServiceField::Name)
    }
}

pub struct TuiApp {
    pub store: mcpstore::MCPStore,
    pub locale: Locale,
    pub active_view: MainView,
    pub service_tab: ServiceManagementTab,
    pub focus_area: FocusArea,
    pub all_services: Vec<ServiceSummary>,
    pub filtered_services: Vec<ServiceSummary>,
    pub selected: usize,
    pub table_state: TableState,
    pub selected_detail: Option<SelectedDetail>,
    pub status_message: String,
    pub status_history: VecDeque<String>,
    pub pending_action: Option<PendingAction>,
    pub edit_modal: Option<EditModalState>,
    pub select_modal: Option<SelectModalState>,
    pub loading_modal: Option<LoadingModalState>,
    pub pending_task: Option<PendingTask>,
    pub should_quit: bool,
    pub tick_rate: Duration,
    pub source_label: String,
    pub cache_storage_label: String,
    pub namespace: String,
    pub config_path: String,
    pub filter: FilterBarState,
    pub service_list_menu: ServiceListMenu,
    pub service_list_pane: ContentPane,
    pub settings_section: SettingsSection,
    pub settings_pane: SettingsPane,
    pub logs_section: LogsSection,
    pub logs_pane: LogsPane,
    pub store_event_history: Vec<String>,
    pub tool_filter: ToolFilterTab,
    pub tool_pane: ContentPane,
    pub tool_services: Vec<ServiceSummary>,
    pub selected_tool_service: usize,
    pub service_tools: Vec<ToolSummary>,
    pub selected_tool: usize,
    pub show_tool_detail: bool,
    pub tool_test_args: String,
    pub tool_test_result: Vec<String>,
    pub agents: Vec<AgentSummary>,
    pub selected_agent: usize,
    pub selected_agent_service: usize,
    pub agent_pane: ContentPane,
    pub pending_agent_id: String,
    pub pending_agent_service: String,
    pub status_cache_lines: Vec<String>,
    pub status_event_lines: Vec<String>,
    pub status_section: StatusSection,
    pub status_pane: ContentPane,
    pub add_service: AddServiceFormState,
    pub show_service_detail: bool,
    last_status_snapshot: String,
}

impl TuiApp {
    pub fn new(
        store: mcpstore::MCPStore,
        tick_rate: Duration,
        locale: Locale,
        source_label: String,
        cache_storage_label: String,
        namespace: String,
        config_path: String,
    ) -> Self {
        let initial_status = "[进行中] 正在加载服务列表".to_string();
        let mut status_history = VecDeque::new();
        status_history.push_back(format_status_history_entry(&initial_status));
        Self {
            store,
            locale,
            active_view: MainView::ServiceManagement,
            service_tab: ServiceManagementTab::Services,
            focus_area: FocusArea::MainNav,
            all_services: Vec::new(),
            filtered_services: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            selected_detail: None,
            status_message: initial_status.clone(),
            status_history,
            pending_action: None,
            edit_modal: None,
            select_modal: None,
            loading_modal: None,
            pending_task: None,
            should_quit: false,
            tick_rate,
            source_label,
            cache_storage_label,
            namespace,
            config_path,
            filter: FilterBarState::default(),
            service_list_menu: ServiceListMenu::All,
            service_list_pane: ContentPane::Menu,
            settings_section: SettingsSection::Status,
            settings_pane: SettingsPane::Menu,
            logs_section: LogsSection::Runtime,
            logs_pane: LogsPane::Menu,
            store_event_history: Vec::new(),
            tool_filter: ToolFilterTab::All,
            tool_pane: ContentPane::Menu,
            tool_services: Vec::new(),
            selected_tool_service: 0,
            service_tools: Vec::new(),
            selected_tool: 0,
            show_tool_detail: false,
            tool_test_args: "{}".to_string(),
            tool_test_result: Vec::new(),
            agents: Vec::new(),
            selected_agent: 0,
            selected_agent_service: 0,
            agent_pane: ContentPane::Menu,
            pending_agent_id: String::new(),
            pending_agent_service: String::new(),
            status_cache_lines: Vec::new(),
            status_event_lines: Vec::new(),
            status_section: StatusSection::Overview,
            status_pane: ContentPane::Menu,
            add_service: AddServiceFormState::default(),
            show_service_detail: false,
            last_status_snapshot: initial_status,
        }
    }

    pub fn sync_status_history(&mut self) {
        if self.last_status_snapshot == self.status_message {
            return;
        }

        self.last_status_snapshot = self.status_message.clone();
        self.status_history
            .push_back(format_status_history_entry(&self.status_message));

        while self.status_history.len() > 120 {
            self.status_history.pop_front();
        }
    }

    pub fn refresh_log_sources(&mut self, rt: &tokio::runtime::Runtime) {
        let events = rt.block_on(async { self.store.event_history(100).await });
        self.store_event_history = events
            .into_iter()
            .map(|event| {
                format!(
                    "{}  {}  {}",
                    format_millis_timestamp(event.timestamp),
                    event.event_type,
                    compact_json(&event.payload, 120)
                )
            })
            .collect();
    }

    pub fn refresh_status_sources(&mut self, rt: &tokio::runtime::Runtime) {
        self.status_cache_lines = match rt.block_on(async { self.store.cache_health_check().await })
        {
            Ok(value) => json_lines(&value, 160),
            Err(error) => vec![format!("cache health error: {error}")],
        };
        let event_capability = rt.block_on(async { self.store.event_capability_report().await });
        self.status_event_lines = json_lines(&event_capability, 160);
    }

    pub fn next_view(&mut self) {
        self.shift_view(1);
    }

    pub fn previous_view(&mut self) {
        self.shift_view(-1);
    }

    pub fn next_service_tab(&mut self) {
        self.shift_service_tab(1);
    }

    pub fn previous_service_tab(&mut self) {
        self.shift_service_tab(-1);
    }

    pub fn select_service_tab(&mut self, tab: ServiceManagementTab) {
        self.service_tab = tab;
        self.service_list_pane = ContentPane::Menu;
        self.add_service.pane = AddServicePane::Menu;
        self.status_message = format!("[进行中] 服务管理: {}", tab.label(self.locale));
    }

    pub fn focus_service_list_menu(&mut self) {
        self.service_list_pane = ContentPane::Menu;
        self.show_service_detail = false;
        self.status_message = "[进行中] 服务列表: 左侧菜单".to_string();
    }

    pub fn focus_service_list_body(&mut self) {
        self.service_list_pane = ContentPane::Body;
        self.status_message = "[进行中] 服务列表: 内容区".to_string();
    }

    pub fn next_service_list_menu_item(&mut self, rt: &tokio::runtime::Runtime) {
        self.shift_service_list_menu(1, rt);
    }

    pub fn previous_service_list_menu_item(&mut self, rt: &tokio::runtime::Runtime) {
        self.shift_service_list_menu(-1, rt);
    }

    pub fn next_settings_section(&mut self) {
        self.shift_settings_section(1);
    }

    pub fn previous_settings_section(&mut self) {
        self.shift_settings_section(-1);
    }

    pub fn next_logs_section(&mut self) {
        self.shift_logs_section(1);
    }

    pub fn previous_logs_section(&mut self) {
        self.shift_logs_section(-1);
    }

    pub fn focus_logs_menu(&mut self) {
        self.logs_pane = LogsPane::Menu;
        self.status_message = "[进行中] 日志: 左侧菜单".to_string();
    }

    pub fn focus_logs_body(&mut self) {
        self.logs_pane = LogsPane::Body;
        self.status_message = "[进行中] 日志: 内容区".to_string();
    }

    pub fn next_tool_filter(&mut self, rt: &tokio::runtime::Runtime) {
        self.shift_tool_filter(1, rt);
    }

    pub fn previous_tool_filter(&mut self, rt: &tokio::runtime::Runtime) {
        self.shift_tool_filter(-1, rt);
    }

    pub fn focus_tool_service_menu(&mut self) {
        self.tool_pane = ContentPane::Menu;
        self.status_message = "[进行中] 工具管理: 服务菜单".to_string();
    }

    pub fn focus_tool_list(&mut self) {
        self.tool_pane = ContentPane::Body;
        self.status_message = "[进行中] 工具管理: 工具列表".to_string();
    }

    pub fn next_tool_service(&mut self, rt: &tokio::runtime::Runtime) {
        self.shift_tool_service(1, rt);
    }

    pub fn previous_tool_service(&mut self, rt: &tokio::runtime::Runtime) {
        self.shift_tool_service(-1, rt);
    }

    pub fn next_tool(&mut self) {
        self.shift_tool_selection(1);
    }

    pub fn previous_tool(&mut self) {
        self.shift_tool_selection(-1);
    }

    pub fn queue_tool_refresh(&mut self) {
        if self.tool_filter != ToolFilterTab::All && self.current_tool_service_name().is_none() {
            self.status_message = "[警告] 当前没有可读取工具的服务".to_string();
            return;
        }

        self.pending_task = Some(PendingTask::RefreshTools);
        self.loading_modal = Some(LoadingModalState {
            title: "读取工具列表".to_string(),
            message: if self.tool_filter == ToolFilterTab::All {
                "正在连接全部服务并读取全局工具列表...".to_string()
            } else {
                "正在连接服务并读取工具列表...".to_string()
            },
        });
        self.status_message = "[进行中] 正在读取工具列表".to_string();
    }

    pub fn open_selected_tool_detail(&mut self) {
        if self.current_tool().is_none() {
            self.status_message = "[警告] 当前服务没有可查看的工具".to_string();
            return;
        }

        self.show_tool_detail = true;
        self.status_message = "[进行中] 查看工具详情".to_string();
    }

    pub fn close_tool_detail(&mut self) {
        self.show_tool_detail = false;
        self.status_message = "[进行中] 已关闭工具详情".to_string();
    }

    pub fn open_tool_test_editor(&mut self) {
        if self.current_tool().is_none() || self.current_tool_service_name().is_none() {
            self.status_message = "[警告] 当前没有可测试的工具".to_string();
            return;
        }

        self.edit_modal = Some(EditModalState {
            target: EditTarget::ToolTestArgs,
            title: "测试工具参数".to_string(),
            value: self.tool_test_args.clone(),
            hint: "输入 JSON object，Enter 执行，Esc 取消".to_string(),
        });
    }

    pub fn focus_agent_menu(&mut self) {
        self.agent_pane = ContentPane::Menu;
        self.status_message = "[进行中] Agent列表: 左侧菜单".to_string();
    }

    pub fn focus_agent_services(&mut self) {
        self.agent_pane = ContentPane::Body;
        self.status_message = "[进行中] Agent列表: 授权服务".to_string();
    }

    pub fn next_agent(&mut self) {
        self.shift_agent(1);
    }

    pub fn previous_agent(&mut self) {
        self.shift_agent(-1);
    }

    pub fn next_agent_service(&mut self) {
        self.shift_agent_service(1);
    }

    pub fn previous_agent_service(&mut self) {
        self.shift_agent_service(-1);
    }

    pub fn queue_agent_refresh(&mut self) {
        self.pending_task = Some(PendingTask::RefreshAgents);
        self.loading_modal = Some(LoadingModalState {
            title: "刷新 Agent".to_string(),
            message: "正在读取 Agent 列表与服务授权关系...".to_string(),
        });
        self.status_message = "[进行中] 正在刷新 Agent 列表".to_string();
    }

    pub fn open_agent_id_editor(&mut self) {
        self.edit_modal = Some(EditModalState {
            target: EditTarget::AgentId,
            title: "选择 Agent".to_string(),
            value: self
                .current_agent_id()
                .map(ToString::to_string)
                .unwrap_or_default(),
            hint: "输入 Agent ID，Enter 保存，Esc 取消".to_string(),
        });
    }

    pub fn open_agent_assign_editor(&mut self) {
        let Some(agent_id) = self.current_agent_id().map(ToString::to_string) else {
            self.open_agent_id_editor();
            return;
        };
        self.pending_agent_id = agent_id;
        self.edit_modal = Some(EditModalState {
            target: EditTarget::AgentAssignService,
            title: "授权服务给 Agent".to_string(),
            value: self
                .all_services
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            hint: "输入服务名称，Enter 授权，Esc 取消".to_string(),
        });
    }

    pub fn queue_agent_unassign(&mut self) {
        let Some(agent_id) = self.current_agent_id().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有 Agent".to_string();
            return;
        };
        let Some(service_name) = self.current_agent_service().map(ToString::to_string) else {
            self.status_message = "[警告] 当前 Agent 没有可解除授权的服务".to_string();
            return;
        };
        self.pending_agent_id = agent_id;
        self.pending_agent_service = service_name;
        self.pending_task = Some(PendingTask::UnassignAgentService);
        self.loading_modal = Some(LoadingModalState {
            title: "解除 Agent 授权".to_string(),
            message: "正在更新 Agent 服务授权关系...".to_string(),
        });
    }

    pub fn focus_status_menu(&mut self) {
        self.status_pane = ContentPane::Menu;
        self.status_message = "[进行中] 状态: 左侧菜单".to_string();
    }

    pub fn focus_status_body(&mut self) {
        self.status_pane = ContentPane::Body;
        self.status_message = "[进行中] 状态: 内容区".to_string();
    }

    pub fn next_status_section(&mut self) {
        self.shift_status_section(1);
    }

    pub fn previous_status_section(&mut self) {
        self.shift_status_section(-1);
    }

    pub fn focus_settings_menu(&mut self) {
        self.settings_pane = SettingsPane::Menu;
        self.status_message = "[进行中] 设置: 左侧菜单".to_string();
    }

    pub fn focus_settings_detail(&mut self) {
        self.settings_pane = SettingsPane::Detail;
        self.status_message = "[进行中] 设置: 右侧内容".to_string();
    }

    pub fn next_add_service_mode(&mut self) {
        self.shift_add_service_mode(1);
    }

    pub fn previous_add_service_mode(&mut self) {
        self.shift_add_service_mode(-1);
    }

    pub fn select_add_service_mode(&mut self, mode: AddServiceMode) {
        self.add_service.mode = mode;
        self.add_service.selected_section = AddServiceMode::MENU
            .iter()
            .position(|item| *item == mode)
            .unwrap_or(0);
        self.add_service.selected_field = 0;
        self.add_service.pane = AddServicePane::Menu;
        self.status_message = format!("[进行中] 添加服务模式: {}", mode.label());
    }

    pub fn focus_add_service_menu(&mut self) {
        self.add_service.pane = AddServicePane::Menu;
        self.status_message = "[进行中] 添加服务: 左侧菜单".to_string();
    }

    pub fn focus_add_service_form(&mut self) {
        self.add_service.pane = AddServicePane::Form;
        self.status_message = "[进行中] 添加服务: 右侧表单".to_string();
    }

    pub fn next_add_service_menu_item(&mut self) {
        self.shift_add_service_section(1);
    }

    pub fn previous_add_service_menu_item(&mut self) {
        self.shift_add_service_section(-1);
    }

    pub fn next_add_service_form_field(&mut self) {
        self.shift_add_service_field(1);
    }

    pub fn previous_add_service_form_field(&mut self) {
        self.shift_add_service_field(-1);
    }

    pub fn open_settings_editor(&mut self) {
        if self.active_view != MainView::Settings
            || self.settings_section != SettingsSection::General
            || self.settings_pane != SettingsPane::Detail
        {
            return;
        }

        self.edit_modal = Some(EditModalState {
            target: EditTarget::Locale,
            title: "编辑语种".to_string(),
            value: self.locale.as_config_value().to_string(),
            hint: "输入 zh-cn 或 en-us，Enter 保存，Esc 取消".to_string(),
        });
    }

    pub fn open_add_service_editor(&mut self) {
        if self.active_view != MainView::ServiceManagement
            || self.service_tab != ServiceManagementTab::AddService
        {
            return;
        }

        let field = self.add_service.selected_field();
        if field == AddServiceField::Submit {
            self.submit_add_service();
            return;
        }

        if let Some(options) = add_service_select_options(field) {
            let value = self.add_service_value(field);
            let selected = options
                .iter()
                .position(|option| option == &value)
                .unwrap_or(0);
            self.select_modal = Some(SelectModalState {
                target: EditTarget::AddServiceField(field),
                title: format!("选择 {}", field.label()),
                options,
                selected,
            });
            return;
        }

        self.edit_modal = Some(EditModalState {
            target: EditTarget::AddServiceField(field),
            title: format!("编辑 {}", field.label()),
            value: self.add_service_value(field),
            hint: add_service_field_hint(field).to_string(),
        });
    }

    pub fn handle_edit_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                if let Some(modal) = self.edit_modal.as_mut() {
                    modal.value.push(c);
                }
            }
            KeyCode::Backspace => {
                if let Some(modal) = self.edit_modal.as_mut() {
                    modal.value.pop();
                }
            }
            KeyCode::Esc => {
                self.edit_modal = None;
                self.status_message = "[进行中] 已取消编辑".to_string();
            }
            KeyCode::Enter => self.save_edit_modal(),
            _ => {}
        }
    }

    pub fn handle_select_input(&mut self, key: KeyEvent) {
        let Some(modal) = self.select_modal.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Char('k') | KeyCode::Up => {
                modal.selected = modal.selected.saturating_sub(1);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !modal.options.is_empty() {
                    modal.selected = (modal.selected + 1).min(modal.options.len() - 1);
                }
            }
            KeyCode::Esc => {
                self.select_modal = None;
                self.status_message = "[进行中] 已取消选择".to_string();
            }
            KeyCode::Enter => self.save_select_modal(),
            _ => {}
        }
    }

    pub fn focus_next_area(&mut self) {
        self.focus_area = self.focus_area.next();
        self.filter.search_mode = false;
        if self.active_view == MainView::ServiceManagement
            && self.focus_area == FocusArea::ViewTable
        {
            self.service_list_pane = ContentPane::Menu;
            self.add_service.pane = AddServicePane::Menu;
        }
        if self.active_view == MainView::Settings && self.focus_area == FocusArea::ViewTable {
            self.settings_pane = SettingsPane::Menu;
        }
        if self.active_view == MainView::Logs && self.focus_area == FocusArea::ViewTable {
            self.logs_pane = LogsPane::Menu;
        }
        if self.active_view == MainView::Tools && self.focus_area == FocusArea::ViewTable {
            self.tool_pane = ContentPane::Menu;
        }
        if self.active_view == MainView::Agents && self.focus_area == FocusArea::ViewTable {
            self.agent_pane = ContentPane::Menu;
        }
        if self.active_view == MainView::Status && self.focus_area == FocusArea::ViewTable {
            self.status_pane = ContentPane::Menu;
        }
        self.status_message = format!("[进行中] 焦点: {}", self.focus_area.label(self.locale));
    }

    pub fn focus_previous_area(&mut self) {
        self.focus_area = self.focus_area.previous();
        self.filter.search_mode = false;
        self.status_message = format!("[进行中] 焦点: {}", self.focus_area.label(self.locale));
    }

    fn shift_view(&mut self, offset: isize) {
        let visible_pages = pages::visible_pages();
        if visible_pages.is_empty() {
            return;
        }

        let current = visible_pages
            .iter()
            .position(|page| page.id == self.active_view)
            .unwrap_or(0) as isize;
        let len = visible_pages.len() as isize;
        let next = (current + offset).rem_euclid(len) as usize;
        self.active_view = visible_pages[next].id;
        self.filter.search_mode = false;
        self.status_message = format!("[进行中] 当前页面: {}", self.active_view.label(self.locale));
    }

    fn shift_service_tab(&mut self, offset: isize) {
        let current = ServiceManagementTab::ALL
            .iter()
            .position(|tab| *tab == self.service_tab)
            .unwrap_or(0) as isize;
        let len = ServiceManagementTab::ALL.len() as isize;
        let next = (current + offset).rem_euclid(len) as usize;
        self.select_service_tab(ServiceManagementTab::ALL[next]);
    }

    fn shift_service_list_menu(&mut self, offset: isize, rt: &tokio::runtime::Runtime) {
        let current = ServiceListMenu::ALL
            .iter()
            .position(|item| *item == self.service_list_menu)
            .unwrap_or(0) as isize;
        let len = ServiceListMenu::ALL.len() as isize;
        let next = (current + offset).clamp(0, len - 1) as usize;
        self.service_list_menu = ServiceListMenu::ALL[next];
        self.apply_filter();
        self.selected = 0;
        self.table_state
            .select(if self.filtered_services.is_empty() {
                None
            } else {
                Some(0)
            });
        if let Err(error) = self.refresh_selected_detail(rt) {
            self.status_message = format!("[错误] {error}");
        } else {
            self.status_message = format!("[进行中] 服务列表: {}", self.service_list_menu.label());
        }
    }

    fn shift_settings_section(&mut self, offset: isize) {
        let current = SettingsSection::ALL
            .iter()
            .position(|section| *section == self.settings_section)
            .unwrap_or(0) as isize;
        let len = SettingsSection::ALL.len() as isize;
        let next = (current + offset).rem_euclid(len) as usize;
        self.settings_section = SettingsSection::ALL[next];
        self.status_message = format!(
            "[进行中] 设置: {}",
            self.settings_section.label(self.locale)
        );
    }

    fn shift_logs_section(&mut self, offset: isize) {
        let current = LogsSection::ALL
            .iter()
            .position(|section| *section == self.logs_section)
            .unwrap_or(0) as isize;
        let len = LogsSection::ALL.len() as isize;
        let next = (current + offset).clamp(0, len - 1) as usize;
        self.logs_section = LogsSection::ALL[next];
        self.status_message = format!("[进行中] 日志: {}", self.logs_section.label(self.locale));
    }

    fn shift_status_section(&mut self, offset: isize) {
        let current = StatusSection::ALL
            .iter()
            .position(|section| *section == self.status_section)
            .unwrap_or(0) as isize;
        let len = StatusSection::ALL.len() as isize;
        let next = (current + offset).clamp(0, len - 1) as usize;
        self.status_section = StatusSection::ALL[next];
        self.status_message = format!("[进行中] 状态: {}", self.status_section.label());
    }

    fn shift_tool_filter(&mut self, offset: isize, rt: &tokio::runtime::Runtime) {
        let current = ToolFilterTab::ALL
            .iter()
            .position(|tab| *tab == self.tool_filter)
            .unwrap_or(0) as isize;
        let len = ToolFilterTab::ALL.len() as isize;
        let next = (current + offset).rem_euclid(len) as usize;
        self.tool_filter = ToolFilterTab::ALL[next];
        self.apply_tool_filter();
        self.selected_tool_service = 0;
        if let Err(error) = self.refresh_tools_for_selected_service(rt, false) {
            self.status_message = format!("[错误] {error}");
            return;
        }
        self.status_message = format!("[进行中] 工具分类: {}", self.tool_filter.label());
    }

    fn shift_tool_service(&mut self, offset: isize, rt: &tokio::runtime::Runtime) {
        if self.tool_services.is_empty() {
            self.selected_tool_service = 0;
            self.service_tools.clear();
            return;
        }

        let len = self.tool_services.len() as isize;
        let next = (self.selected_tool_service as isize + offset).clamp(0, len - 1);
        self.selected_tool_service = next as usize;
        self.selected_tool = 0;
        if let Err(error) = self.refresh_tools_for_selected_service(rt, false) {
            self.status_message = format!("[错误] {error}");
            return;
        }
        if let Some(name) = self.current_tool_service_name() {
            self.status_message = format!("[进行中] 工具服务: {name}");
        }
    }

    fn shift_tool_selection(&mut self, offset: isize) {
        if self.service_tools.is_empty() {
            self.selected_tool = 0;
            return;
        }

        let len = self.service_tools.len() as isize;
        let next = (self.selected_tool as isize + offset).clamp(0, len - 1);
        self.selected_tool = next as usize;
        if let Some(tool) = self.current_tool() {
            self.status_message = format!("[进行中] 当前工具: {}", tool.name);
        }
    }

    fn shift_agent(&mut self, offset: isize) {
        if self.agents.is_empty() {
            self.selected_agent = 0;
            self.selected_agent_service = 0;
            return;
        }

        let len = self.agents.len() as isize;
        let next = (self.selected_agent as isize + offset).clamp(0, len - 1);
        self.selected_agent = next as usize;
        self.selected_agent_service = 0;
        if let Some(agent) = self.current_agent_id() {
            self.status_message = format!("[进行中] 当前 Agent: {agent}");
        }
    }

    fn shift_agent_service(&mut self, offset: isize) {
        let Some(agent) = self.current_agent() else {
            self.selected_agent_service = 0;
            return;
        };
        if agent.services.is_empty() {
            self.selected_agent_service = 0;
            return;
        }

        let len = agent.services.len() as isize;
        let next = (self.selected_agent_service as isize + offset).clamp(0, len - 1);
        self.selected_agent_service = next as usize;
        if let Some(service) = self.current_agent_service() {
            self.status_message = format!("[进行中] Agent 授权服务: {service}");
        }
    }

    fn shift_add_service_mode(&mut self, offset: isize) {
        let current = AddServiceMode::ALL
            .iter()
            .position(|mode| *mode == self.add_service.mode)
            .unwrap_or(0) as isize;
        let len = AddServiceMode::ALL.len() as isize;
        let next = (current + offset).rem_euclid(len) as usize;
        self.add_service.mode = AddServiceMode::ALL[next];
        self.add_service.selected_section = AddServiceMode::MENU
            .iter()
            .position(|item| *item == self.add_service.mode)
            .unwrap_or(0);
        self.add_service.selected_field = 0;
        self.add_service.pane = AddServicePane::Menu;
        self.status_message = format!("[进行中] 添加服务模式: {}", self.add_service.mode.label());
    }

    fn shift_add_service_section(&mut self, offset: isize) {
        let current = AddServiceMode::MENU
            .iter()
            .position(|mode| *mode == self.add_service.mode)
            .unwrap_or(0) as isize;
        let len = AddServiceMode::MENU.len() as isize;
        let next = (current + offset).clamp(0, len - 1);
        self.add_service.selected_section = next as usize;
        self.add_service.mode = AddServiceMode::MENU[next as usize];
        self.add_service.selected_field = 0;
        self.status_message = format!("[进行中] 添加服务: {}", self.add_service.mode.menu_label());
    }

    fn shift_add_service_field(&mut self, offset: isize) {
        let len = self.add_service.selected_fields().len() as isize;
        if len == 0 {
            self.add_service.selected_field = 0;
            return;
        }
        let next = (self.add_service.selected_field as isize + offset).clamp(0, len - 1);
        self.add_service.selected_field = next as usize;
        self.status_message = format!(
            "[进行中] 添加服务字段: {}",
            self.add_service.selected_field().label()
        );
    }

    fn save_edit_modal(&mut self) {
        let Some(modal) = self.edit_modal.take() else {
            return;
        };

        match modal.target {
            EditTarget::Locale => {
                let Some(locale) = Locale::from_config_value(&modal.value) else {
                    self.status_message = "[错误] 语种只支持 zh-cn 或 en-us".to_string();
                    self.edit_modal = Some(modal);
                    return;
                };

                let manager = self.store.config_manager();
                let mut config = match manager.load_app_config_or_default() {
                    Ok(config) => config,
                    Err(error) => {
                        self.status_message = format!("[错误] 读取配置失败: {error}");
                        self.edit_modal = Some(modal);
                        return;
                    }
                };
                config.ui.language = locale.as_config_value().to_string();

                if let Err(error) = manager.save_app_config(&config) {
                    self.status_message = format!("[错误] 保存配置失败: {error}");
                    self.edit_modal = Some(modal);
                    return;
                }

                self.locale = locale;
                self.status_message = "[成功] 已保存语种配置".to_string();
            }
            EditTarget::AddServiceField(field) => {
                self.set_add_service_value(field, modal.value);
                self.status_message = format!("[成功] 已更新字段 {}", field.label());
            }
            EditTarget::ToolTestArgs => {
                let parsed = match serde_json::from_str::<serde_json::Value>(&modal.value) {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        self.status_message = format!("[错误] 工具测试参数不是合法 JSON: {error}");
                        self.edit_modal = Some(modal);
                        return;
                    }
                };
                if !parsed.is_object() {
                    self.status_message = "[错误] 工具测试参数必须是 JSON object".to_string();
                    self.edit_modal = Some(modal);
                    return;
                }

                self.tool_test_args = modal.value;
                self.pending_task = Some(PendingTask::ToolTest);
                self.loading_modal = Some(LoadingModalState {
                    title: "测试工具".to_string(),
                    message: "正在调用工具并等待结果...".to_string(),
                });
                self.status_message = "[进行中] 正在测试工具".to_string();
            }
            EditTarget::AgentId => {
                let agent_id = modal.value.trim();
                if agent_id.is_empty() {
                    self.status_message = "[错误] Agent ID 不能为空".to_string();
                    self.edit_modal = Some(modal);
                    return;
                }
                self.ensure_agent_visible(agent_id.to_string());
                self.status_message = format!("[成功] 已选择 Agent {agent_id}");
            }
            EditTarget::AgentAssignService => {
                let service_name = modal.value.trim();
                if service_name.is_empty() {
                    self.status_message = "[错误] 服务名称不能为空".to_string();
                    self.edit_modal = Some(modal);
                    return;
                }
                self.pending_agent_service = service_name.to_string();
                self.pending_task = Some(PendingTask::AssignAgentService);
                self.loading_modal = Some(LoadingModalState {
                    title: "Agent 服务授权".to_string(),
                    message: "正在更新 Agent 服务授权关系...".to_string(),
                });
                self.status_message = "[进行中] 正在授权服务给 Agent".to_string();
            }
        }
    }

    fn save_select_modal(&mut self) {
        let Some(modal) = self.select_modal.take() else {
            return;
        };

        let Some(value) = modal.options.get(modal.selected).cloned() else {
            return;
        };

        match modal.target {
            EditTarget::Locale => {}
            EditTarget::AddServiceField(field) => {
                self.set_add_service_value(field, value);
                self.status_message = format!("[成功] 已选择字段 {}", field.label());
            }
            EditTarget::ToolTestArgs => {}
            EditTarget::AgentId | EditTarget::AgentAssignService => {}
        }
    }

    fn add_service_value(&self, field: AddServiceField) -> String {
        match field {
            AddServiceField::Name => self.add_service.name.clone(),
            AddServiceField::Command => self.add_service.command.clone(),
            AddServiceField::Args => self.add_service.args.clone(),
            AddServiceField::Url => self.add_service.url.clone(),
            AddServiceField::Description => self.add_service.description.clone(),
            AddServiceField::WorkingDir => self.add_service.working_dir.clone(),
            AddServiceField::Env => self.add_service.env.clone(),
            AddServiceField::Headers => self.add_service.headers.clone(),
            AddServiceField::Scope => self.add_service.scope.clone(),
            AddServiceField::Agent => self.add_service.agent.clone(),
            AddServiceField::ConnectAfterAdd => self.add_service.connect_after_add.clone(),
            AddServiceField::Json => self.add_service.json.clone(),
            AddServiceField::Toml => self.add_service.toml.clone(),
            AddServiceField::Submit => String::new(),
        }
    }

    fn set_add_service_value(&mut self, field: AddServiceField, value: String) {
        match field {
            AddServiceField::Name => self.add_service.name = value,
            AddServiceField::Command => self.add_service.command = value,
            AddServiceField::Args => self.add_service.args = value,
            AddServiceField::Url => self.add_service.url = value,
            AddServiceField::Description => self.add_service.description = value,
            AddServiceField::WorkingDir => self.add_service.working_dir = value,
            AddServiceField::Env => self.add_service.env = value,
            AddServiceField::Headers => self.add_service.headers = value,
            AddServiceField::Scope => self.add_service.scope = value.to_ascii_lowercase(),
            AddServiceField::Agent => self.add_service.agent = value,
            AddServiceField::ConnectAfterAdd => {
                self.add_service.connect_after_add = value.to_ascii_lowercase()
            }
            AddServiceField::Json => self.add_service.json = value,
            AddServiceField::Toml => self.add_service.toml = value,
            AddServiceField::Submit => {}
        }
    }

    pub fn submit_add_service(&mut self) {
        if self.active_view != MainView::ServiceManagement
            || self.service_tab != ServiceManagementTab::AddService
        {
            return;
        }

        if let Err(error) = self.build_add_service_config() {
            self.status_message = format!("[错误] {error}");
            return;
        }

        self.pending_task = Some(PendingTask::AddService);
        self.loading_modal = Some(LoadingModalState {
            title: "添加服务".to_string(),
            message: "正在写入配置、连接服务并刷新服务列表...".to_string(),
        });
        self.status_message = "[进行中] 正在添加服务".to_string();
    }

    pub fn has_pending_task(&self) -> bool {
        self.pending_task.is_some()
    }

    pub fn process_pending_task(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(task) = self.pending_task.take() else {
            return Ok(());
        };

        let result = match task {
            PendingTask::AddService => self.execute_add_service(rt),
            PendingTask::RefreshTools => self.execute_refresh_tools(rt),
            PendingTask::ToolTest => self.execute_tool_test(rt),
            PendingTask::RefreshAgents => self.execute_refresh_agents(rt),
            PendingTask::AssignAgentService => self.execute_assign_agent_service(rt),
            PendingTask::UnassignAgentService => self.execute_unassign_agent_service(rt),
        };

        self.loading_modal = None;
        result
    }

    fn execute_add_service(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let (name, config, scope, agent) =
            self.build_add_service_config().map_err(add_service_error)?;
        let transport = config.infer_transport().to_string();
        let connect_after_add =
            parse_yes_no(&self.add_service.connect_after_add).map_err(add_service_error)?;

        let stored_name = if scope == "agent" {
            rt.block_on(async {
                self.store
                    .add_service_for_agent(&agent, &name, config)
                    .await
            })?
        } else {
            rt.block_on(async { self.store.add_service(&name, config).await })?;
            name.clone()
        };

        let connect_result = if connect_after_add {
            Some(rt.block_on(async { self.store.connect_service(&stored_name).await }))
        } else {
            None
        };

        self.refresh(rt, false)?;
        self.select_service_by_name(&stored_name, rt)?;
        self.active_view = MainView::ServiceManagement;
        self.service_tab = ServiceManagementTab::Services;
        self.service_list_pane = ContentPane::Body;
        self.focus_area = FocusArea::ViewTable;
        self.show_service_detail = false;
        let service_label = if stored_name == name {
            name
        } else {
            format!("{name} -> {stored_name}")
        };

        self.status_message = match connect_result {
            Some(Ok(())) => {
                format!("[成功] 已添加并连接服务 {service_label} (transport={transport})")
            }
            Some(Err(error)) => {
                format!("[错误] 已添加服务 {service_label}，但连接失败: {error}")
            }
            None => {
                format!("[成功] 已添加服务 {service_label} (未自动连接, transport={transport})")
            }
        };
        Ok(())
    }

    fn execute_refresh_tools(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        self.refresh(rt, false)?;
        self.refresh_tools_for_selected_service(rt, true)?;
        let service = self.current_tool_service_name().unwrap_or("-").to_string();
        self.status_message = format!(
            "[成功] 已读取工具列表 {service} (tools={})",
            self.service_tools.len()
        );
        Ok(())
    }

    fn execute_tool_test(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let service = self
            .current_tool_call_service_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "当前没有可测试的服务"))?
            .to_string();
        let tool = self
            .current_tool()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "当前没有可测试的工具"))?
            .original_name
            .clone();
        let args: serde_json::Value = serde_json::from_str(&self.tool_test_args)?;
        let result = rt.block_on(async { self.store.call_tool(&service, &tool, args).await })?;

        self.tool_test_result = format_tool_call_result(result.is_error, &result.content);
        self.show_tool_detail = true;
        self.refresh(rt, false)?;
        self.refresh_tools_for_selected_service(rt, false)?;
        self.status_message = format!("[成功] 工具测试完成 {service}/{tool}");
        Ok(())
    }

    fn execute_refresh_agents(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        self.refresh_agents(rt)?;
        self.status_message = format!("[成功] 已刷新 Agent 列表 (agents={})", self.agents.len());
        Ok(())
    }

    fn execute_assign_agent_service(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let agent_id =
            trim_required(&self.pending_agent_id, "Agent ID").map_err(add_service_error)?;
        let service_name = trim_required(&self.pending_agent_service, "Service name")
            .map_err(add_service_error)?;
        rt.block_on(async {
            self.store
                .assign_service_to_agent(&agent_id, &service_name)
                .await
        })?;
        self.refresh_agents(rt)?;
        self.status_message = format!("[成功] 已授权服务 {service_name} 给 Agent {agent_id}");
        Ok(())
    }

    fn execute_unassign_agent_service(
        &mut self,
        rt: &tokio::runtime::Runtime,
    ) -> Result<(), BoxErr> {
        let agent_id =
            trim_required(&self.pending_agent_id, "Agent ID").map_err(add_service_error)?;
        let service_name = trim_required(&self.pending_agent_service, "Service name")
            .map_err(add_service_error)?;
        rt.block_on(async {
            self.store
                .unassign_service_from_agent(&agent_id, &service_name)
                .await
        })?;
        self.refresh_agents(rt)?;
        self.status_message = format!("[成功] 已解除 Agent {agent_id} 的服务 {service_name}");
        Ok(())
    }

    fn build_add_service_config(&self) -> Result<(String, ServerConfig, String, String), String> {
        let name = trim_required(&self.add_service.name, "Name")?;
        let scope = normalized_scope(&self.add_service.scope)?;
        let agent = self.add_service.agent.trim().to_string();
        if scope == "agent" && agent.is_empty() {
            return Err("Agent scope requires Agent ID".to_string());
        }

        let config = match self.add_service.mode {
            AddServiceMode::Stdio => self.build_stdio_config()?,
            AddServiceMode::Http => self.build_http_config()?,
            AddServiceMode::Json => serde_json::from_str::<ServerConfig>(&self.add_service.json)
                .map_err(|error| format!("Invalid ServerConfig JSON: {error}"))?,
            AddServiceMode::Toml => toml::from_str::<ServerConfig>(&self.add_service.toml)
                .map_err(|error| format!("Invalid ServerConfig TOML: {error}"))?,
        };

        Ok((name, config, scope, agent))
    }

    fn build_stdio_config(&self) -> Result<ServerConfig, String> {
        let command_line = trim_required(&self.add_service.command, "Command")?;
        let mut command_parts = split_words(&command_line);
        if command_parts.is_empty() {
            return Err("Command cannot be empty".to_string());
        }

        let command = command_parts.remove(0);
        let mut args = command_parts;
        args.extend(split_words(&self.add_service.args));

        Ok(ServerConfig {
            url: None,
            command: Some(command),
            args,
            env: parse_kv_items(&self.add_service.env, "Env vars")?,
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: trim_optional(&self.add_service.working_dir),
            description: trim_optional(&self.add_service.description),
            mcpstore: None,
        })
    }

    fn build_http_config(&self) -> Result<ServerConfig, String> {
        let url = trim_required(&self.add_service.url, "URL")?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("HTTP mode requires http:// or https:// URL".to_string());
        }

        Ok(ServerConfig {
            url: Some(url),
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            headers: parse_kv_items(&self.add_service.headers, "Headers")?,
            transport: Some("streamable-http".to_string()),
            working_dir: None,
            description: trim_optional(&self.add_service.description),
            mcpstore: None,
        })
    }

    pub fn header_stats(&self) -> HeaderStats {
        let total = self.all_services.len();
        let connected = self
            .all_services
            .iter()
            .filter(|s| s.status == ConnectionStatus::Connected)
            .count();
        let error = self
            .all_services
            .iter()
            .filter(|s| s.status == ConnectionStatus::Error)
            .count();
        let connecting = self
            .all_services
            .iter()
            .filter(|s| s.status == ConnectionStatus::Connecting)
            .count();
        let disconnected = self
            .all_services
            .iter()
            .filter(|s| s.status == ConnectionStatus::Disconnected)
            .count();

        HeaderStats {
            total,
            connected,
            error,
            connecting,
            disconnected,
            cache_storage: self.cache_storage_label.clone(),
            namespace: self.namespace.clone(),
            config_path: self.config_path.clone(),
        }
    }

    pub fn refresh(
        &mut self,
        rt: &tokio::runtime::Runtime,
        reload_source: bool,
    ) -> Result<(), BoxErr> {
        if reload_source {
            rt.block_on(async { self.store.load_from_source().await })?;
        }

        let services = rt.block_on(async { self.store.list_services().await });
        self.all_services = services.into_iter().map(ServiceSummary::from).collect();
        self.apply_filter();
        self.apply_tool_filter();

        let selected_name = self.current_service_name().map(ToString::to_string);
        self.selected = match selected_name {
            Some(name) => self
                .filtered_services
                .iter()
                .position(|s| s.name == name)
                .unwrap_or(0),
            None => 0,
        };

        if self.filtered_services.is_empty() {
            self.selected = 0;
            self.table_state.select(None);
            self.selected_detail = None;
        } else {
            if self.selected >= self.filtered_services.len() {
                self.selected = self.filtered_services.len() - 1;
            }
            self.table_state.select(Some(self.selected));
            self.refresh_selected_detail(rt)?;
        }

        if self.active_view == MainView::Tools {
            self.refresh_tools_for_selected_service(rt, false)?;
        }
        if self.active_view == MainView::Agents {
            self.refresh_agents(rt)?;
        }
        if self.active_view == MainView::Status {
            self.refresh_status_sources(rt);
        }

        Ok(())
    }

    fn apply_filter(&mut self) {
        self.filtered_services = filter_and_sort(&self.all_services, &self.filter)
            .into_iter()
            .filter(|service| self.service_list_menu.matches(&service.transport))
            .collect();
    }

    fn apply_tool_filter(&mut self) {
        let selected_name = self.current_tool_service_name().map(ToString::to_string);
        self.tool_services = if self.tool_filter == ToolFilterTab::All {
            Vec::new()
        } else {
            self.all_services
                .iter()
                .filter(|service| self.tool_filter.matches(service))
                .cloned()
                .collect()
        };

        self.selected_tool_service = selected_name
            .and_then(|name| {
                self.tool_services
                    .iter()
                    .position(|service| service.name == name)
            })
            .unwrap_or(0);

        if self.selected_tool_service >= self.tool_services.len() {
            self.selected_tool_service = self.tool_services.len().saturating_sub(1);
        }
    }

    pub fn current_service_name(&self) -> Option<&str> {
        self.filtered_services
            .get(self.selected)
            .map(|s| s.name.as_str())
    }

    pub fn current_tool_service(&self) -> Option<&ServiceSummary> {
        if self.tool_filter == ToolFilterTab::All {
            return None;
        }
        self.tool_services.get(self.selected_tool_service)
    }

    pub fn current_tool_service_name(&self) -> Option<&str> {
        self.current_tool_service()
            .map(|service| service.name.as_str())
    }

    pub fn current_tool(&self) -> Option<&ToolSummary> {
        self.service_tools.get(self.selected_tool)
    }

    pub fn current_tool_call_service_name(&self) -> Option<&str> {
        self.current_tool()
            .map(|tool| tool.global_service_name.as_str())
            .or_else(|| self.current_tool_service_name())
    }

    pub fn current_agent(&self) -> Option<&AgentSummary> {
        self.agents.get(self.selected_agent)
    }

    pub fn current_agent_id(&self) -> Option<&str> {
        self.current_agent().map(|agent| agent.id.as_str())
    }

    pub fn current_agent_service(&self) -> Option<&str> {
        self.current_agent()
            .and_then(|agent| agent.services.get(self.selected_agent_service))
            .map(String::as_str)
    }

    pub fn refresh_tools_for_selected_service(
        &mut self,
        rt: &tokio::runtime::Runtime,
        connect: bool,
    ) -> Result<(), BoxErr> {
        if self.tool_filter == ToolFilterTab::All {
            if connect {
                for service in self.all_services.clone() {
                    rt.block_on(async { self.store.connect_service(&service.name).await })
                        .ok();
                }
                self.refresh(rt, false)?;
            }

            let tools = rt.block_on(async { self.store.list_tools_scoped(None, None).await })?;
            self.service_tools = tools
                .into_iter()
                .filter_map(ToolSummary::from_payload)
                .collect();
            if self.selected_tool >= self.service_tools.len() {
                self.selected_tool = self.service_tools.len().saturating_sub(1);
            }
            return Ok(());
        }

        let Some(name) = self.current_tool_service_name().map(ToString::to_string) else {
            self.service_tools.clear();
            self.selected_tool = 0;
            return Ok(());
        };

        if connect {
            rt.block_on(async { self.store.connect_service(&name).await })?;
        }

        let tools = rt.block_on(async { self.store.list_tools(&name).await })?;
        self.service_tools = tools
            .into_iter()
            .map(|tool| ToolSummary {
                name: tool.name.clone(),
                original_name: tool.name,
                service_name: name.clone(),
                global_service_name: name.clone(),
                description: tool.description,
                input_schema: tool.input_schema,
            })
            .collect();

        if self.selected_tool >= self.service_tools.len() {
            self.selected_tool = self.service_tools.len().saturating_sub(1);
        }

        Ok(())
    }

    pub fn refresh_agents(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let selected_id = self.current_agent_id().map(ToString::to_string);
        let agents = rt.block_on(async { self.store.list_agents().await })?;
        self.agents = agents
            .into_iter()
            .filter_map(AgentSummary::from_value)
            .collect();
        self.agents.sort_by(|left, right| left.id.cmp(&right.id));

        self.selected_agent = selected_id
            .and_then(|id| self.agents.iter().position(|agent| agent.id == id))
            .unwrap_or(0);
        if self.selected_agent >= self.agents.len() {
            self.selected_agent = self.agents.len().saturating_sub(1);
        }
        if let Some(agent) = self.current_agent() {
            if self.selected_agent_service >= agent.services.len() {
                self.selected_agent_service = agent.services.len().saturating_sub(1);
            }
        } else {
            self.selected_agent_service = 0;
        }
        Ok(())
    }

    fn ensure_agent_visible(&mut self, agent_id: String) {
        if let Some(index) = self.agents.iter().position(|agent| agent.id == agent_id) {
            self.selected_agent = index;
            self.selected_agent_service = 0;
            return;
        }
        self.agents.push(AgentSummary {
            id: agent_id.clone(),
            services: Vec::new(),
        });
        self.agents.sort_by(|left, right| left.id.cmp(&right.id));
        self.selected_agent = self
            .agents
            .iter()
            .position(|agent| agent.id == agent_id)
            .unwrap_or(0);
        self.selected_agent_service = 0;
    }

    pub fn refresh_selected_detail(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.selected_detail = None;
            return Ok(());
        };
        let Some(service) = self.filtered_services.get(self.selected) else {
            self.selected_detail = None;
            return Ok(());
        };

        let status = match rt.block_on(async { self.store.cached_service_status(&name).await })? {
            Some(status) => Some(status),
            None => rt
                .block_on(async { self.store.health_check(&name).await })
                .ok(),
        };

        let detail = if let Some(status) = status {
            SelectedDetail {
                title: service.name.clone(),
                transport: service.transport.clone(),
                endpoint: service.endpoint.clone(),
                scope: if service.agent_id == "global_agent_store" {
                    "store".to_string()
                } else {
                    format!("agent: {}", service.agent_id)
                },
                added_time: format_timestamp(service.added_time),
                connection_status: format_connection_status(service.status).to_string(),
                health_status: format!("{:?}", status.health_status),
                attempts: format!(
                    "{}/{}",
                    status.connection_attempts, status.max_connection_attempts
                ),
                latency: format_latency(status.latency_p95, status.latency_p99),
                retry_time: status
                    .next_retry_time
                    .map(format_retry_time)
                    .unwrap_or_else(|| "-".to_string()),
                error_message: status.current_error.unwrap_or_else(|| "-".to_string()),
                tools: status
                    .tools
                    .into_iter()
                    .map(|tool| format!("{} [{:?}]", tool.tool_original_name, tool.status))
                    .collect(),
            }
        } else {
            SelectedDetail {
                title: service.name.clone(),
                transport: service.transport.clone(),
                endpoint: service.endpoint.clone(),
                scope: if service.agent_id == "global_agent_store" {
                    "store".to_string()
                } else {
                    format!("agent: {}", service.agent_id)
                },
                added_time: format_timestamp(service.added_time),
                connection_status: format_connection_status(service.status).to_string(),
                health_status: "-".to_string(),
                attempts: "-".to_string(),
                latency: "-".to_string(),
                retry_time: "-".to_string(),
                error_message: "-".to_string(),
                tools: Vec::new(),
            }
        };

        self.selected_detail = Some(detail);
        Ok(())
    }

    pub fn open_selected_detail(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        if self.current_service_name().is_none() {
            self.status_message = "[警告] 当前没有可查看的服务".to_string();
            return Ok(());
        }
        self.refresh_selected_detail(rt)?;
        self.show_service_detail = self.selected_detail.is_some();
        self.status_message = "[进行中] 查看服务详情".to_string();
        Ok(())
    }

    pub fn close_service_detail(&mut self) {
        self.show_service_detail = false;
        self.status_message = "[进行中] 已关闭服务详情".to_string();
    }

    fn select_service_by_name(
        &mut self,
        name: &str,
        rt: &tokio::runtime::Runtime,
    ) -> Result<(), BoxErr> {
        let Some(index) = self
            .filtered_services
            .iter()
            .position(|service| service.name == name)
        else {
            return Ok(());
        };

        self.selected = index;
        self.table_state.select(Some(index));
        self.refresh_selected_detail(rt)
    }

    pub fn move_selection(
        &mut self,
        offset: isize,
        rt: &tokio::runtime::Runtime,
    ) -> Result<(), BoxErr> {
        if self.filtered_services.is_empty() {
            return Ok(());
        }
        let len = self.filtered_services.len() as isize;
        let next = (self.selected as isize + offset).clamp(0, len - 1);
        self.selected = next as usize;
        self.table_state.select(Some(self.selected));
        self.refresh_selected_detail(rt)?;
        Ok(())
    }

    pub fn jump_to(&mut self, index: usize, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        if self.filtered_services.is_empty() {
            return Ok(());
        }
        self.selected = index.min(self.filtered_services.len() - 1);
        self.table_state.select(Some(self.selected));
        self.refresh_selected_detail(rt)?;
        Ok(())
    }

    pub fn handle_search_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                self.filter.search_text.push(c);
                self.apply_filter();
                self.selected = 0;
                self.table_state.select(Some(0));
            }
            KeyCode::Backspace => {
                self.filter.search_text.pop();
                self.apply_filter();
                self.selected = 0;
                self.table_state.select(Some(0));
            }
            KeyCode::Esc => {
                self.filter.search_mode = false;
            }
            _ => {}
        }
    }

    pub fn set_status_filter(&mut self, status: FilterStatus, rt: &tokio::runtime::Runtime) {
        self.filter.active_status = status;
        self.apply_filter();
        self.selected = 0;
        self.table_state.select(Some(0));
        if let Err(e) = self.refresh_selected_detail(rt) {
            self.status_message = format!("[错误] {e}");
        }
    }

    pub fn toggle_sort(&mut self) {
        self.filter.sort_by = self.filter.sort_by.next();
        self.apply_filter();
    }

    pub fn toggle_sort_direction(&mut self) {
        self.filter.sort_asc = !self.filter.sort_asc;
        self.apply_filter();
    }

    pub fn connect_selected(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
            return Ok(());
        };
        rt.block_on(async { self.store.connect_service(&name).await })?;
        self.refresh(rt, false)?;
        self.status_message = format!("[成功] 已连接服务 {name}");
        Ok(())
    }

    pub fn disconnect_selected(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
            return Ok(());
        };
        rt.block_on(async { self.store.disconnect_service(&name).await })?;
        self.refresh(rt, false)?;
        self.status_message = format!("[成功] 已断开服务 {name}");
        Ok(())
    }

    pub fn restart_selected(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
            return Ok(());
        };
        rt.block_on(async { self.store.restart_service(&name).await })?;
        self.refresh(rt, false)?;
        self.status_message = format!("[成功] 已重启服务 {name}");
        Ok(())
    }

    pub fn prompt_remove(&mut self) {
        if let Some(name) = self.current_service_name().map(ToString::to_string) {
            self.pending_action = Some(PendingAction::Remove(name.clone()));
            self.status_message = format!("[警告] 确认删除服务 {name}？按 y 确认，按 n 取消");
        } else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
        }
    }

    pub fn confirm_remove(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        if let Some(PendingAction::Remove(name)) = self.pending_action.take() {
            rt.block_on(async { self.store.remove_service(&name).await })?;
            self.refresh(rt, false)?;
            self.status_message = format!("[成功] 已删除服务 {name}");
        }
        Ok(())
    }

    pub fn cancel_pending(&mut self) {
        self.pending_action = None;
        self.status_message = "[进行中] 已取消当前操作".to_string();
    }
}

pub struct TerminalGuard;

impl TerminalGuard {
    pub fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen).ok();
    }
}

pub fn run(
    args: &StoreSourceArgs,
    tick_ms: u64,
    locale_override: Option<Locale>,
) -> Result<(), BoxErr> {
    bootstrap::init_tracing_silent("mcpstore=warn");

    let rt = bootstrap::build_runtime()?;
    let store = crate::store_args::build_store(args)?;
    rt.block_on(async { store.load_from_source().await })?;

    let app_config = store.config_manager().load_app_config_or_default()?;
    let locale = locale_override
        .or_else(|| Locale::from_config_value(&app_config.ui.language))
        .unwrap_or_default();
    let cache_storage_label =
        rt.block_on(async { store.current_cache_storage().await.as_str().to_string() });
    let namespace = store.namespace();
    let config_path = store.config_manager().mcp_path().display().to_string();

    let mut app = TuiApp::new(
        store,
        Duration::from_millis(tick_ms),
        locale,
        args.source.as_str().to_string(),
        cache_storage_label,
        namespace,
        config_path,
    );
    app.refresh(&rt, false)?;
    app.status_message = "[成功] TUI 已就绪，按 q 退出".to_string();

    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| super::ui::draw(frame, &mut app, &rt))?;
        if app.has_pending_task() {
            if let Err(error) = app.process_pending_task(&rt) {
                app.status_message = format!("[错误] {error}");
            }
            continue;
        }

        if app.should_quit {
            break;
        }

        if event::poll(app.tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    if let Err(error) = super::events::handle_key(&mut app, &rt, key) {
                        app.status_message = format!("[错误] {error}");
                    }
                }
            }
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

fn format_connection_status(status: ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Connected => "connected",
        ConnectionStatus::Connecting => "connecting",
        ConnectionStatus::Disconnected => "disconnected",
        ConnectionStatus::Error => "error",
    }
}

fn format_latency(p95: Option<f64>, p99: Option<f64>) -> String {
    match (p95, p99) {
        (Some(p95), Some(p99)) => format!("p95={p95:.0}ms  p99={p99:.0}ms"),
        _ => "-".to_string(),
    }
}

fn format_retry_time(timestamp: f64) -> String {
    let seconds = timestamp.trunc() as i64;
    let fractional = (timestamp.fract() * 1_000_000_000.0) as u32;
    chrono::DateTime::from_timestamp(seconds, fractional)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn format_timestamp(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "-".to_string();
    }
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn format_status_history_entry(message: &str) -> String {
    format!("{}  {}", chrono::Local::now().format("%H:%M:%S"), message)
}

fn format_millis_timestamp(timestamp: i64) -> String {
    let seconds = timestamp.div_euclid(1000);
    let millis = timestamp.rem_euclid(1000) as u32;
    chrono::DateTime::from_timestamp(seconds, millis * 1_000_000)
        .map(|time| time.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn compact_json(value: &serde_json::Value, limit: usize) -> String {
    let text = if value.is_null() {
        "-".to_string()
    } else {
        value.to_string()
    };
    truncate_chars(&text, limit)
}

fn json_lines(value: &serde_json::Value, limit: usize) -> Vec<String> {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|_| "{}".to_string())
        .lines()
        .map(|line| truncate_chars(line, limit))
        .collect()
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() && limit > 3 {
        format!("{}...", head.chars().take(limit - 3).collect::<String>())
    } else {
        head
    }
}

fn format_tool_call_result(is_error: bool, content: &[ContentItem]) -> Vec<String> {
    let mut lines = vec![format!("is_error={is_error}")];
    if content.is_empty() {
        lines.push("-".to_string());
        return lines;
    }

    for item in content {
        match item {
            ContentItem::Text { text, .. } => lines.push(truncate_chars(text, 180)),
            ContentItem::Image { mime_type, .. } => lines.push(format!("[Image: {mime_type}]")),
            ContentItem::Audio { mime_type, .. } => lines.push(format!("[Audio: {mime_type}]")),
            ContentItem::Resource { resource, .. } => lines.push(format!(
                "[Resource: {}]",
                truncate_chars(&resource.to_string(), 160)
            )),
            ContentItem::ResourceLink { resource, .. } => lines.push(format!(
                "[ResourceLink: {}]",
                truncate_chars(&resource.to_string(), 160)
            )),
        }
    }

    lines
}

fn add_service_field_hint(field: AddServiceField) -> &'static str {
    match field {
        AddServiceField::Name => "服务名称，Enter 保存，Esc 取消",
        AddServiceField::Command => "stdio 命令，可包含参数，Enter 保存，Esc 取消",
        AddServiceField::Args => "额外参数，按空格分隔，Enter 保存，Esc 取消",
        AddServiceField::Url => "http:// 或 https:// URL，Enter 保存，Esc 取消",
        AddServiceField::Description => "可选描述，Enter 保存，Esc 取消",
        AddServiceField::WorkingDir => "可选工作目录，Enter 保存，Esc 取消",
        AddServiceField::Env => "KEY=VALUE，可用逗号分隔多项，Enter 保存，Esc 取消",
        AddServiceField::Headers => "KEY=VALUE，可用逗号分隔多项，Enter 保存，Esc 取消",
        AddServiceField::Scope => "store 或 agent，Enter 保存，Esc 取消",
        AddServiceField::Agent => "scope=agent 时必填，Enter 保存，Esc 取消",
        AddServiceField::ConnectAfterAdd => "yes/no，是否添加后立即连接，Enter 保存，Esc 取消",
        AddServiceField::Json => "ServerConfig JSON，Enter 保存，Esc 取消",
        AddServiceField::Toml => "ServerConfig TOML，Enter 保存，Esc 取消",
        AddServiceField::Submit => "提交添加服务",
    }
}

fn add_service_select_options(field: AddServiceField) -> Option<Vec<String>> {
    match field {
        AddServiceField::Scope => Some(vec!["store".to_string(), "agent".to_string()]),
        AddServiceField::ConnectAfterAdd => Some(vec!["no".to_string(), "yes".to_string()]),
        _ => None,
    }
}

fn trim_required(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{label} cannot be empty"))
    } else {
        Ok(trimmed.to_string())
    }
}

fn trim_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalized_scope(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "store" => Ok("store".to_string()),
        "agent" => Ok("agent".to_string()),
        _ => Err("Scope must be store or agent".to_string()),
    }
}

fn parse_yes_no(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "no" | "n" | "false" | "0" => Ok(false),
        "yes" | "y" | "true" | "1" => Ok(true),
        _ => Err("Connect after add must be yes or no".to_string()),
    }
}

fn split_words(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|item| !item.trim().is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_kv_items(value: &str, label: &str) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for item in value
        .split(|c| c == ',' || c == '\n')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let Some((key, val)) = item.split_once('=') else {
            return Err(format!("{label} item '{item}' must be KEY=VALUE"));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("{label} contains an empty key"));
        }
        out.insert(key.to_string(), val.trim().to_string());
    }
    Ok(out)
}

fn add_service_error(error: String) -> BoxErr {
    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_add_service_key_value_items() {
        let parsed = parse_kv_items("TOKEN=abc, DEBUG=true", "env").unwrap();
        assert_eq!(parsed.get("TOKEN").map(String::as_str), Some("abc"));
        assert_eq!(parsed.get("DEBUG").map(String::as_str), Some("true"));
    }

    #[test]
    fn parses_add_service_yes_no_values() {
        assert_eq!(parse_yes_no("yes"), Ok(true));
        assert_eq!(parse_yes_no("0"), Ok(false));
        assert!(parse_yes_no("maybe").is_err());
    }
}
