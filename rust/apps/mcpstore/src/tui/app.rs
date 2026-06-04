use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mcpstore::registry::ConnectionStatus;
use ratatui::{
    backend::CrosstermBackend,
    widgets::TableState, Terminal,
};

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

pub struct TuiApp {
    pub store: mcpstore::MCPStore,
    pub all_services: Vec<ServiceSummary>,
    pub filtered_services: Vec<ServiceSummary>,
    pub selected: usize,
    pub table_state: TableState,
    pub selected_detail: Option<SelectedDetail>,
    pub status_message: String,
    pub pending_action: Option<PendingAction>,
    pub should_quit: bool,
    pub tick_rate: Duration,
    pub source_label: String,
    pub backend_label: String,
    pub namespace: String,
    pub config_path: String,
    pub filter: FilterBarState,
}

impl TuiApp {
    pub fn new(
        store: mcpstore::MCPStore,
        tick_rate: Duration,
        source_label: String,
        backend_label: String,
        namespace: String,
        config_path: String,
    ) -> Self {
        Self {
            store,
            all_services: Vec::new(),
            filtered_services: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            selected_detail: None,
            status_message: "[进行中] 正在加载服务列表".to_string(),
            pending_action: None,
            should_quit: false,
            tick_rate,
            source_label,
            backend_label,
            namespace,
            config_path,
            filter: FilterBarState::default(),
        }
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
            backend: self.backend_label.clone(),
            namespace: self.namespace.clone(),
            config_path: self.config_path.clone(),
        }
    }

    pub fn refresh(&mut self, rt: &tokio::runtime::Runtime, reload_source: bool) -> Result<(), BoxErr> {
        if reload_source {
            rt.block_on(async { self.store.load_from_source().await })?;
        }

        let services = rt.block_on(async { self.store.list_services().await });
        self.all_services = services.into_iter().map(ServiceSummary::from).collect();
        self.apply_filter();

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

        Ok(())
    }

    fn apply_filter(&mut self) {
        self.filtered_services = filter_and_sort(&self.all_services, &self.filter);
    }

    pub fn current_service_name(&self) -> Option<&str> {
        self.filtered_services
            .get(self.selected)
            .map(|s| s.name.as_str())
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
                attempts: format!("{}/{}", status.connection_attempts, status.max_connection_attempts),
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

    pub fn move_selection(&mut self, offset: isize, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
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

pub fn run(args: &StoreSourceArgs, tick_ms: u64) -> Result<(), BoxErr> {
    bootstrap::init_tracing("mcpstore=warn");

    let rt = bootstrap::build_runtime()?;
    let store = crate::store_args::build_store(args)?;
    rt.block_on(async { store.load_from_source().await })?;

    let backend_label = rt.block_on(async { store.current_backend().await.as_str().to_string() });
    let namespace = store.namespace();
    let config_path = store.config_manager().mcp_path().display().to_string();

    let mut app = TuiApp::new(
        store,
        Duration::from_millis(tick_ms),
        args.source.as_str().to_string(),
        backend_label,
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
